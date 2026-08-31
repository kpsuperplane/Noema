#[test]
fn foreground_browser_owner_is_conversation_scoped() {
    let first = test_turn();
    let mut later = test_turn();
    later.turn_id = "turn:later".to_string();
    let mut separate = test_turn();
    separate.conversation_id = "conversation:separate".to_string();

    assert_eq!(
        super::browse_owner_key_for_turn(&first),
        super::browse_owner_key_for_turn(&later),
    );
    assert_ne!(
        super::browse_owner_key_for_turn(&first),
        super::browse_owner_key_for_turn(&separate),
    );
}

async fn set_browser_snapshot_for_test(
    actor: &RuntimeActor,
    owner: &str,
    revision: u64,
    reference: &str,
    role: &str,
    name: &str,
) {
    set_browser_snapshot_with_submission_for_test(
        actor, owner, revision, reference, role, name, None,
    )
    .await;
}

async fn set_browser_snapshot_with_submission_for_test(
    actor: &RuntimeActor,
    owner: &str,
    revision: u64,
    reference: &str,
    role: &str,
    name: &str,
    submission: Option<noema_capabilities::web::browse::BrowseSubmissionContext>,
) {
    let route = crate::daemon::runtime::web_tools::resolve_web_browse_route(&actor.store)
        .await
        .expect("browser route");
    let backend = noema_providers::WebBrowseBackendHandle::obscura(1, 64);
    let element = noema_capabilities::web::browse::BrowseInteractiveElement {
        reference: reference.to_string(),
        role: role.to_string(),
        name: name.to_string(),
        href: None,
        disabled: false,
        submission,
    };
    actor.browser_sessions.set_session(
        owner.to_string(),
        crate::daemon::runtime::actor::BrowserSessionState {
            route,
            active_position: 0,
            backend,
            last_navigation_url: Some("https://example.com/form".to_string()),
            public_revision: revision,
            backend_revision: revision,
            snapshot: Some(crate::daemon::runtime::actor::BrowserSnapshotContext {
                url: "https://example.com/form".to_string(),
                title: "Newsletter".to_string(),
                revision,
                elements: HashMap::from([(reference.to_string(), element)]),
            }),
        },
    );
}

#[test]
fn browser_public_revisions_do_not_repeat_after_session_removal() {
    let coordinator = crate::daemon::runtime::actor::BrowserSessionCoordinator::default();
    assert_eq!(coordinator.next_revision("conversation:revision"), 1);
    coordinator.remove("conversation:revision");
    assert_eq!(coordinator.next_revision("conversation:revision"), 2);
}

#[tokio::test]
async fn browser_review_values_stay_out_of_tool_results() {
    let actor = test_actor().await;
    let route = crate::daemon::runtime::web_tools::resolve_web_browse_route(&actor.store)
        .await
        .expect("browser route");
    let submission = noema_capabilities::web::browse::BrowseSubmissionContext {
        destination: "https://example.com/submit".to_string(),
        method: "POST".to_string(),
        fields: vec![noema_capabilities::web::browse::BrowseSubmissionField {
            name: "amount".to_string(),
            value: "125.00".to_string(),
        }],
        omitted_control_count: 0,
        truncated: false,
    };
    let mut response = noema_capabilities::web::browse::BrowseResponse {
        provider: "obscura".to_string(),
        state: "open".to_string(),
        snapshot: Some(noema_capabilities::web::browse::BrowseSnapshot {
            url: "https://example.com/form".to_string(),
            title: "Form".to_string(),
            text: "Form".to_string(),
            snapshot_revision: 3,
            elements: vec![noema_capabilities::web::browse::BrowseInteractiveElement {
                reference: "e1".to_string(),
                role: "button".to_string(),
                name: "Submit".to_string(),
                href: None,
                disabled: false,
                submission: Some(submission.clone()),
            }],
            truncated: false,
        }),
        screenshot: None,
    };
    let mut state = crate::daemon::runtime::actor::BrowserSessionState {
        route,
        active_position: 0,
        backend: noema_providers::WebBrowseBackendHandle::obscura(1, 64),
        last_navigation_url: Some("https://example.com/form".to_string()),
        public_revision: 0,
        backend_revision: 0,
        snapshot: None,
    };

    super::update_browser_snapshot_authority(&mut state, &mut response, 7);

    assert_eq!(response.snapshot.expect("result snapshot").elements[0].submission, None);
    assert_eq!(
        state.snapshot.expect("review snapshot").elements["e1"].submission,
        Some(submission)
    );
}

#[tokio::test]
async fn browser_switch_retries_the_next_route_after_a_failed_open() {
    let actor = test_actor().await;
    let mut route = crate::daemon::runtime::web_tools::resolve_web_browse_route(&actor.store)
        .await
        .expect("browser route");
    route.providers.push(route.providers[0].clone());
    route.providers.push(route.providers[0].clone());
    let state = crate::daemon::runtime::actor::BrowserSessionState {
        route,
        active_position: 0,
        backend: noema_providers::WebBrowseBackendHandle::obscura(1, 64),
        last_navigation_url: Some("https://example.com/failed".to_string()),
        public_revision: 1,
        backend_revision: 1,
        snapshot: None,
    };

    assert_eq!(super::next_browser_route_position(&state), Some(1));
    assert_eq!(super::validate_browser_switch_revision(&state, None), Ok(()));
    assert_eq!(
        super::validate_browser_switch_revision(&state, Some(1)),
        Err(noema_providers::WebBrowseError::StaleSnapshot)
    );
}

#[tokio::test]
async fn browser_interactions_without_a_live_backend_fail_before_action_review() {
    let actor = test_actor().await;
    let mut turn = test_turn();
    turn.initial_model_tools = test_governed_web_browse_model_tools();
    let result = actor
        .execute_local_tool(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &test_tool_call(
                noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
                json!({"snapshot_revision":1,"ref":"e6","action":"fill","value":"Call parents"}),
            ),
        )
        .await;

    assert!(!result.success);
    assert!(result.blocked_action_request.is_none());
    assert_eq!(result.payload["error"], "session_not_found");
    assert_eq!(
        result.payload["message"],
        "this conversation or task has no active browser session"
    );
    assert!(
        actor
            .store
            .list_pending_governed_actions("human:local", None, None, 10)
            .await
            .expect("pending actions")
            .is_empty(),
    );
}

#[tokio::test]
async fn browser_snapshot_validation_rejects_stale_revision_and_missing_target() {
    let actor = test_actor().await;
    let owner = "conversation:current";
    set_browser_snapshot_for_test(
        &actor,
        owner,
        2,
        "e6",
        "input",
        "Next calendar event",
    )
    .await;

    assert_eq!(
        actor.validate_browser_snapshot_call(
            owner,
            noema_capabilities::web::browse::parse_command(
                noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
                &json!({"snapshot_revision":1,"ref":"e6","action":"fill","value":"Call parents"}),
            )
            .expect("interaction"),
        ),
        Err(noema_providers::WebBrowseError::StaleSnapshot),
    );
    assert_eq!(
        actor.validate_browser_snapshot_call(
            owner,
            noema_capabilities::web::browse::parse_command(
                noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
                &json!({"snapshot_revision":2,"ref":"e7","action":"fill","value":"Call parents"}),
            )
            .expect("interaction"),
        ),
        Err(noema_providers::WebBrowseError::ElementNotFound),
    );
}

#[tokio::test]
async fn web_fetch_runtime_context_uses_only_available_saved_summarizer_selection() {
    let store = crate::test_support::test_store().await;
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation");
    upsert_ready_auxiliary_model_preference(
        &store,
        NewAuxiliaryModelPreference {
            task: AuxiliaryModelTask::WebFetchSummarizer,
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
                model_profile: "default".to_string(),
                reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
            },
            fast_mode: false,
        },
    )
    .await;
    let selected_actor = RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([
            (
                "codex".to_string(),
                local_tool_test_provider(),
            ),
            (
                "foundation_local".to_string(),
                local_tool_test_provider(),
            ),
        ]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor");

    let context = selected_actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Background)
        .await
        .expect("web fetch context");

    assert_eq!(
        context.summarizer_route.selection().provider_kind,
        "foundation_local",
    );
    assert_eq!(context.summarizer_model, "default");
    assert_eq!(
        context.summarizer_reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
    assert_eq!(
        context.generation_priority,
        noema_providers::GenerationPriority::Background
    );

    let missing_provider_actor = test_actor_with_store(&store).await;
    let message = missing_provider_actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect_err("missing provider should fail");

    assert_eq!(
        message,
        "web.fetch summarizer provider is not available in this daemon"
    );
}

#[tokio::test]
async fn browser_approval_persists_page_and_target_review_context() {
    let mut actor = test_actor().await;
    let mut turn = test_turn();
    let current_route = actor
        .resolve_primary_provider()
        .await
        .expect("primary route");
    turn.provider_kind = current_route.selection().provider_kind.clone();
    turn.model = current_route.selection().model_profile.clone();
    turn.provider_route = Arc::new(current_route);
    let (conversation_id, turn_id, item_id) =
        crate::contract_test_support::seed_authorization_source(&actor.store, "Submit the form.")
            .await;
    turn.conversation_id = conversation_id;
    turn.turn_id = turn_id;
    turn.user_item_id = item_id;
    turn.initial_model_tools = test_governed_web_browse_model_tools();
    let owner = super::browse_owner_key_for_turn(&turn);
    set_browser_snapshot_with_submission_for_test(
        &actor,
        &owner,
        3,
        "e8",
        "button",
        "Submit",
        Some(noema_capabilities::web::browse::BrowseSubmissionContext {
            destination: "https://example.com/submit".to_string(),
            method: "POST".to_string(),
            fields: vec![noema_capabilities::web::browse::BrowseSubmissionField {
                name: "amount".to_string(),
                value: "125.00".to_string(),
            }],
            omitted_control_count: 1,
            truncated: false,
        }),
    )
    .await;

    let call = test_tool_call(
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
        json!({"snapshot_revision":3,"ref":"e8","action":"click"}),
    );
    let binding = turn
        .initial_model_tools
        .bindings
        .resolve(&call.name)
        .expect("browser interaction binding");
    let preparation = actor
        .prepare_reviewed_action(
            &turn,
            &AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            &call,
            binding,
        )
        .await;
    let action_id = match preparation.expect("review preparation") {
        super::super::action_gateway::ReviewedActionPreparation::AwaitingApproval(action) => {
            action.action_id
        }
        _ => panic!("browser interaction must await approval"),
    };
    let action = actor
        .store
        .get_governed_action(&action_id, 1)
        .await
        .expect("read action")
        .expect("action");
    assert_eq!(
        super::browse_owner_key_for_action(&action).as_deref(),
        Some(owner.as_str()),
    );
    assert_eq!(
        action.authorization_context["browser_review_context"],
        json!({
            "kind": "browser_interaction",
            "page": {"url":"https://example.com/form","title":"Newsletter"},
            "target": {
                "ref":"e8",
                "role":"button",
                "name":"Submit",
                "href":null,
                "submission": {
                    "destination":"https://example.com/submit",
                    "method":"POST",
                    "fields":[{"name":"amount","value":"125.00"}],
                    "omitted_control_count":1,
                    "truncated":false
                }
            }
        })
    );
    let resolved = actor
        .resolve_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("resolve stale browser approval");
    assert_eq!(resolved.state, noema_store::GovernedActionState::Superseded);
    assert_eq!(
        resolved.failure_code.as_deref(),
        Some("browser_session_unavailable")
    );
    for _ in 0..2 {
        let closed = actor
            .execute_web_browse_action(
                owner.clone(),
                None,
                noema_capabilities::web::browse::WEB_BROWSE_CLOSE_TOOL,
                &json!({}),
                "test",
            )
            .await;
        assert!(closed.success);
    }
    assert!(actor.browser_sessions.session(&owner).is_none());
}

#[tokio::test]
async fn nonrepeatable_browser_effect_cannot_return_with_a_new_snapshot() {
    let actor = test_actor().await;
    let (task, _) = crate::test_support::seed_task(&actor.store, "Submit the form.").await;
    let service = noema_store::WorkCommandService::new(
        actor.store.clone(),
        actor.provider_registry.clone(),
    );
    let claimed = service
        .claim_next_work_run("worker:declined-browser-effect", 60, &[])
        .await
        .expect("claim Task run")
        .expect("Task run");
    let fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
    };
    service
        .start_work_run(
            &fence,
            "actor:test:runtime",
            None,
            "correlation:declined-browser-effect",
        )
        .await
        .expect("start Task run");

    let current_route = actor
        .resolve_primary_provider()
        .await
        .expect("primary route");
    let mut turn = test_turn();
    turn.provider_kind = current_route.selection().provider_kind.clone();
    turn.model = current_route.selection().model_profile.clone();
    turn.provider_route = Arc::new(current_route);
    turn.task_id = Some(task.task_id.to_string());
    turn.task_run_id = Some(fence.run_id.clone());
    turn.task_run_fence = Some(fence.clone());
    turn.initial_model_tools = test_governed_web_browse_model_tools();
    let agent_identity = AgentPromptIdentity {
        agent_id: "agent:primary".to_string(),
        display_name: None,
    };
    let owner = super::browse_owner_key_for_turn(&turn);
    let submission = noema_capabilities::web::browse::BrowseSubmissionContext {
        destination: "https://example.com/submit".to_string(),
        method: "POST".to_string(),
        fields: vec![noema_capabilities::web::browse::BrowseSubmissionField {
            name: "amount".to_string(),
            value: "125.00".to_string(),
        }],
        omitted_control_count: 0,
        truncated: false,
    };
    set_browser_snapshot_with_submission_for_test(
        &actor,
        &owner,
        3,
        "e8",
        "button",
        "Submit",
        Some(submission.clone()),
    )
    .await;

    let first_call = test_tool_call(
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
        json!({"snapshot_revision": 3, "ref": "e8", "action": "click", "value": ""}),
    );
    let binding = turn
        .initial_model_tools
        .bindings
        .resolve(&first_call.name)
        .expect("browser interaction binding");
    let first = actor
        .prepare_reviewed_action(
            &turn,
            &agent_identity,
            &first_call,
            binding,
        )
        .await
        .expect("first review");
    let super::super::action_gateway::ReviewedActionPreparation::AwaitingApproval(action) = first
    else {
        panic!("first action must await approval");
    };
    actor
        .store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Decline,
        )
        .await
        .expect("decline action");
    let _ = service
        .resume_after_governed_action(
            &action.action_id,
            action.revision,
            "actor:test:runtime",
        )
        .await
        .expect("resume Task")
        .expect("continuation run");
    let claimed = service
        .claim_next_work_run("worker:declined-browser-effect-continuation", 60, &[])
        .await
        .expect("claim continuation")
        .expect("continuation");
    let continuation_fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
    };
    service
        .start_work_run(
            &continuation_fence,
            "actor:test:runtime",
            None,
            "correlation:declined-browser-effect-continuation",
        )
        .await
        .expect("start continuation");
    turn.task_run_id = Some(continuation_fence.run_id.clone());
    turn.task_run_fence = Some(continuation_fence);

    set_browser_snapshot_with_submission_for_test(
        &actor,
        &owner,
        9,
        "e19",
        "button",
        "Submit",
        Some(submission.clone()),
    )
    .await;
    let retry_call = test_tool_call(
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
        json!({"snapshot_revision": 9, "ref": "e19", "action": "click"}),
    );
    let retry = actor
        .prepare_reviewed_action(
            &turn,
            &agent_identity,
            &retry_call,
            binding,
        )
        .await
        .expect("retry review");
    assert!(matches!(
        retry,
        super::super::action_gateway::ReviewedActionPreparation::NonrepeatableEquivalent(
            blocked
        ) if blocked.action_id == action.action_id
    ));

    let mut changed_submission = submission;
    changed_submission.fields[0].value = "130.00".to_string();
    set_browser_snapshot_with_submission_for_test(
        &actor,
        &owner,
        10,
        "e20",
        "button",
        "Submit",
        Some(changed_submission.clone()),
    )
    .await;
    let changed_call = test_tool_call(
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
        json!({"snapshot_revision": 10, "ref": "e20", "action": "click"}),
    );
    let changed = actor
        .prepare_reviewed_action(
            &turn,
            &agent_identity,
            &changed_call,
            binding,
        )
        .await
        .expect("changed effect review");
    let super::super::action_gateway::ReviewedActionPreparation::AwaitingApproval(
        uncertain_action,
    ) = changed
    else {
        panic!("changed effect must create a new action request");
    };
    actor
        .store
        .decide_governed_action(
            &uncertain_action.action_id,
            uncertain_action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("approve changed effect");
    actor
        .store
        .claim_governed_action_execution(
            &uncertain_action.action_id,
            uncertain_action.revision,
            None,
        )
        .await
        .expect("claim changed effect");
    actor
        .store
        .finish_governed_action_execution(
            &uncertain_action.action_id,
            uncertain_action.revision,
            noema_store::GovernedExecutionOutcome::OutcomeUncertain,
            None,
            Some("outcome_uncertain"),
        )
        .await
        .expect("record uncertain effect");
    let _ = service
        .resume_after_governed_action(
            &uncertain_action.action_id,
            uncertain_action.revision,
            "actor:test:runtime",
        )
        .await
        .expect("resume uncertain effect")
        .expect("reconciliation run");
    let claimed = service
        .claim_next_work_run("worker:uncertain-browser-effect-continuation", 60, &[])
        .await
        .expect("claim reconciliation run")
        .expect("reconciliation run");
    let reconciliation_fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
    };
    service
        .start_work_run(
            &reconciliation_fence,
            "actor:test:runtime",
            None,
            "correlation:uncertain-browser-effect-continuation",
        )
        .await
        .expect("start reconciliation run");
    let admitted = service
        .admit_work_run_execution_context(
            &reconciliation_fence,
            "actor:test:runtime",
            None,
            "correlation:uncertain-browser-effect-admit",
        )
        .await
        .expect("admit uncertain action result");
    assert!(admitted.context.lineage.iter().any(|item| {
        item.payload["governed_action"]["state"] == "outcome_uncertain"
    }));
    turn.task_run_id = Some(reconciliation_fence.run_id.clone());
    turn.task_run_fence = Some(reconciliation_fence);

    set_browser_snapshot_with_submission_for_test(
        &actor,
        &owner,
        11,
        "e21",
        "button",
        "Submit",
        Some(changed_submission),
    )
    .await;
    let uncertain_retry_call = test_tool_call(
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
        json!({"snapshot_revision": 11, "ref": "e21", "action": "click"}),
    );
    let uncertain_retry = actor
        .prepare_reviewed_action(
            &turn,
            &agent_identity,
            &uncertain_retry_call,
            binding,
        )
        .await
        .expect("uncertain effect retry review");
    assert!(matches!(
        uncertain_retry,
        super::super::action_gateway::ReviewedActionPreparation::NonrepeatableEquivalent(
            blocked
        ) if blocked.action_id == uncertain_action.action_id
    ));
}

#[tokio::test]
async fn approved_runtime_browser_action_is_not_treated_as_removed_connector() {
    let mut actor = test_actor().await;
    let (conversation_id, turn_id, _) =
        crate::contract_test_support::seed_authorization_source(&actor.store, "Open the page.")
            .await;
    let capability_name = noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL;
    let spec = noema_capabilities::web::browse::tool_specs()
        .expect("browser specs")
        .into_iter()
        .find(|spec| spec.name.as_str() == capability_name)
        .expect("interaction spec");
    let destination =
        super::super::web_tools::resolve_web_destination(&actor.store, capability_name)
            .await
            .expect("browser destination");
    let action = actor
        .store
        .create_governed_action(noema_store::NewGovernedAction {
            owner_human_id: "human:local".to_string(),
            conversation_id: Some(conversation_id),
            turn_id: Some(turn_id),
            task_id: None,
            run_id: None,
            requesting_agent_id: "agent:primary".to_string(),
            capability_name: capability_name.to_string(),
            operation_token: capability_name.to_string(),
            review_route: noema_store::ExecutionReviewRoute::LlmReview,
            behavior: noema_store::StoredToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: true,
            },
            arguments: json!({"snapshot_revision":1,"ref":"e8","action":"click"}),
            input_schema: spec.input_schema.as_value().clone(),
            authorization_context: json!({
                "execution_decision": "llm_review",
                "destination": destination,
            }),
            safe_summary: "Click an element on the open browser page".to_string(),
        })
        .await
        .expect("browser action");
    let action = actor
        .store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            noema_store::NewGovernedActionAssessment {
                status: noema_store::GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                reason_codes: vec!["test_requires_approval".to_string()],
                explanation: "Require the approval path.".to_string(),
            },
            None,
        )
        .await
        .expect("approval request");
    let action = actor
        .store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("approved action");

    let resolved = actor
        .resolve_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            noema_store::GovernedActionDecision::Approve,
        )
        .await
        .expect("approved browser action");

    assert_eq!(resolved.state, noema_store::GovernedActionState::Failed);
    assert_eq!(
        resolved.failure_code.as_deref(),
        Some("tool_declared_failure")
    );
}

#[tokio::test]
async fn web_fetch_runtime_context_no_preference_summarizes_with_spec_default_model() {
    let store = crate::test_support::test_store().await;
    let actor = test_actor_with_store(&store).await;

    let context = actor
        .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect("initialized web fetch context");

    assert_eq!(context.summarizer_model, "gpt-5.6-luna");
    assert_eq!(context.summarizer_route.selection().provider_kind, "codex");
    assert_eq!(
        context.generation_priority,
        noema_providers::GenerationPriority::Foreground
    );
}

#[tokio::test]
async fn bound_exa_web_search_without_secret_falls_back_to_duckduckgo() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = create_exa_provider_account(
        &store,
        noema_providers::ProviderAccountStatus::Authenticated,
    )
    .await;
    insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
    let actor = test_actor_with_store(&store).await;

    let (provider, fallback_from, fallback_reason, auth_failure_account_id) = actor
        .web_search_runtime_provider_resolution()
        .await
        .expect("provider");

    assert_eq!(
        provider.backend_id(),
        noema_providers::DUCKDUCKGO_PUBLIC_PROVIDER_ID,
    );
    assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
    assert_eq!(fallback_reason.as_deref(), Some("provider account unauthenticated"));
    assert!(auth_failure_account_id.is_none());
    let account = store
        .get_provider_account(&provider_account_id)
        .await
        .expect("provider account")
        .expect("Exa account");
    assert_eq!(
        account.status,
        noema_providers::ProviderAccountStatus::Unauthenticated
    );
    assert_eq!(account.last_error_code.as_deref(), Some("auth_failed"));
}

#[tokio::test]
async fn bound_exa_web_fetch_without_secret_falls_back_to_direct_http() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = create_exa_provider_account(
        &store,
        noema_providers::ProviderAccountStatus::Authenticated,
    )
    .await;
    insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
    let actor = test_actor_with_store(&store).await;

    let (provider, context, fallback_from, fallback_reason, auth_failure_account_id) = actor
        .web_fetch_runtime_execution_context(noema_providers::GenerationPriority::Foreground)
        .await
        .expect("context");

    assert_eq!(
        provider.backend_id(),
        noema_providers::DIRECT_HTTP_PROVIDER_ID,
    );
    assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
    assert_eq!(
        fallback_reason.as_deref(),
        Some("provider account unauthenticated")
    );
    assert!(auth_failure_account_id.is_none());
    assert_eq!(context.summarizer_model, "gpt-5.6-luna");
}

#[tokio::test]
async fn web_tool_result_payloads_preserve_fallback_metadata() {
    let store = crate::test_support::test_store().await;
    let provider_account_id = create_exa_provider_account(
        &store,
        noema_providers::ProviderAccountStatus::Authenticated,
    )
    .await;
    insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
    insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
    store
        .update_provider_account_status(
            &provider_account_id,
            noema_providers::ProviderAccountStatus::Unknown,
            None,
            None,
        )
        .await
        .expect("make Exa unavailable");
    let actor = test_actor_with_store(&store).await;

    for (name, arguments, reason, error) in [
        (
            "web.search",
            json!({"query": "   "}),
            "bound provider capability web.search is account_dependent",
            "query is required",
        ),
        (
            "web.fetch",
            json!({"url": ""}),
            "bound provider capability web.fetch is account_dependent",
            "url is required",
        ),
    ] {
        let result = actor
            .execute_local_tool(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(name, arguments),
            )
            .await;

        let GenerateActionItem::ToolResult { payload, .. } =
            super::local_tool_result_action_item(&result)
        else {
            panic!("expected tool result action item");
        };
        assert_eq!(payload["fallback_from"], provider_account_id);
        assert_eq!(payload["fallback_reason"], reason);
        assert_eq!(payload["error"], error);
    }
}
