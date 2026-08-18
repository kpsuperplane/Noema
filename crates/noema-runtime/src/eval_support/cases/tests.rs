use noema_providers::{GenerateInput, NoemaToolChoice, ReasoningEffort};

use super::{evaluation_cases, evaluation_cases_for_roles};
use crate::eval_support::{RuntimeEvalRole, runtime_eval_case_descriptors_for_roles};

#[test]
fn onboarding_case_requires_the_name_tool_for_an_unnamed_agent() {
    let cases = evaluation_cases("local-model").expect("cases");
    assert_eq!(cases.len(), 32, "qualification request contract changed");
    let request = &cases
        .iter()
        .find(|case| case.id == "agent_onboarding_name")
        .expect("onboarding case")
        .request;
    let GenerateInput::Messages(messages) = &request.input else {
        panic!("onboarding case should use message input");
    };

    assert_eq!(
        messages.last().map(|message| message.content.as_str()),
        Some("Momo!")
    );
    assert!(messages[0].content.contains("display_name"));
    assert!(messages[0].content.contains("null"));
    assert!(messages[2].content.contains("update_own_name"));
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].name.as_str(), "update_own_name");
    assert_eq!(request.tool_choice, NoemaToolChoice::Required);
}

#[test]
fn suite_assigns_every_case_to_one_of_the_nine_model_settings() {
    let cases = evaluation_cases("local-model").expect("cases");
    let ids = cases
        .iter()
        .map(|case| case.id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        ids.len(),
        cases.len(),
        "case ids must remain stable and unique"
    );
    assert_eq!(RuntimeEvalRole::ALL.len(), 9);
    for role in RuntimeEvalRole::ALL {
        let count = cases
            .iter()
            .filter(|case| {
                case.role == *role
                    || case.category == crate::eval_support::OPENROUTER_PROTOCOL_CATEGORY
            })
            .count();
        assert!(count >= 5, "{role} has only {count} applicable cases");
    }
    for case_id in [
        "primary_stateful_flight_to_calendar",
        "primary_stateful_public_event_to_calendar",
        "primary_stateful_email_meeting_to_calendar",
        "primary_stateful_email_reschedule",
        "primary_stateful_package_delivery",
        "primary_stateful_passport_reminder",
        "primary_stateful_missing_appointment",
    ] {
        let case = cases
            .iter()
            .find(|case| case.id == case_id && case.role == RuntimeEvalRole::Primary)
            .unwrap_or_else(|| panic!("missing primary stateful-action case {case_id}"));
        assert!(case.critical, "{case_id} must remain non-compensable");
        assert_eq!(case.category, "stateful_action");
        assert!(
            case.request.tools.len() >= 50,
            "{case_id} needs a broad catalog"
        );
        assert!(case.request.tools.iter().all(|tool| {
            tool.exposed_name().chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        }));
    }
    for (case_id, prompt) in [
        (
            "primary_stateful_flight_to_calendar",
            "Can you add AS385 on Sep 17 to my calendar?",
        ),
        (
            "primary_stateful_public_event_to_calendar",
            "Put the Northstar Data Summit opening keynote on my calendar.",
        ),
        (
            "primary_stateful_email_meeting_to_calendar",
            "Put my Rowan Labs interview on my calendar.",
        ),
        (
            "primary_stateful_email_reschedule",
            "Make sure my calendar has the latest time for my Rowan Labs interview.",
        ),
        (
            "primary_stateful_package_delivery",
            "When are my new headphones getting here?",
        ),
        (
            "primary_stateful_passport_reminder",
            "Make sure I don't miss the passport renewal deadline from that email.",
        ),
        (
            "primary_stateful_missing_appointment",
            "Put the dentist appointment from my latest email on my calendar.",
        ),
    ] {
        let case = cases
            .iter()
            .find(|case| case.id == case_id)
            .unwrap_or_else(|| panic!("missing stateful case {case_id}"));
        let GenerateInput::Messages(messages) = &case.request.input else {
            panic!("{case_id} should use message input");
        };
        assert_eq!(
            messages.last().map(|message| message.content.as_str()),
            Some(prompt),
            "stateful prompts should request outcomes without prescribing tools"
        );
    }
    for (role, case_id) in [
        (RuntimeEvalRole::TaskSimple, "task_planner_simple_finish"),
        (RuntimeEvalRole::TaskMedium, "task_planner_finish"),
        (
            RuntimeEvalRole::TaskDifficult,
            "task_planner_difficult_finish",
        ),
    ] {
        assert!(
            cases
                .iter()
                .any(|case| case.id == case_id && case.role == role)
        );
    }
}

#[test]
fn role_subset_runs_shared_protocol_cases_once() {
    let cases = evaluation_cases_for_roles("local-model", &[RuntimeEvalRole::ActionReviewer], None)
        .expect("action reviewer cases");
    assert_eq!(cases.len(), 6);
    assert_eq!(
        cases
            .iter()
            .filter(|case| { case.category == crate::eval_support::OPENROUTER_PROTOCOL_CATEGORY })
            .count(),
        4
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.role == RuntimeEvalRole::ActionReviewer)
            .count(),
        2
    );

    let browser_case = cases
        .iter()
        .find(|case| case.id == "action_reviewer_browser_consent_rejection")
        .expect("browser consent review case");
    let GenerateInput::Text(input) = &browser_case.request.input else {
        panic!("action reviewer case should use text input");
    };
    let input: serde_json::Value = serde_json::from_str(input).expect("reviewer input JSON");
    assert_eq!(
        input["verified_context"]["browser_session"]["storage_lifetime"],
        "session_only"
    );
}

#[test]
fn stateful_cases_reserve_every_provider_round() {
    let descriptors =
        runtime_eval_case_descriptors_for_roles("local-model", &[RuntimeEvalRole::Primary], None)
            .expect("descriptors");
    assert_eq!(
        descriptors
            .iter()
            .filter(|case| case.category == "stateful_action")
            .map(|case| case.maximum_provider_calls)
            .collect::<Vec<_>>(),
        [4, 4, 4, 5, 3, 4, 3]
    );
    assert!(
        descriptors
            .iter()
            .filter(|case| case.category != "stateful_action")
            .all(|case| case.maximum_provider_calls == 1)
    );
}

#[test]
fn task_tier_cases_receive_candidate_reasoning_effort() {
    for case_id in [
        "task_executor_finish",
        "task_executor_medium_finish",
        "task_executor_difficult_finish",
    ] {
        let cases = evaluation_cases_for_roles(
            "local-model",
            &[
                RuntimeEvalRole::TaskSimple,
                RuntimeEvalRole::TaskMedium,
                RuntimeEvalRole::TaskDifficult,
            ],
            Some(ReasoningEffort::High),
        )
        .expect("cases");
        let case = cases
            .iter()
            .find(|case| case.id == case_id)
            .expect("tier case");
        assert_eq!(
            case.request.options.reasoning_effort,
            Some(ReasoningEffort::High),
            "candidate reasoning effort must reach every request"
        );
    }
}

#[test]
fn task_cases_render_current_task_documents_and_terminals() {
    let cases = evaluation_cases("local-model").expect("cases");
    for (case_id, expected_markers) in [
        (
            "task_planner_finish",
            [
                "Authenticated source request:\nFind good hikes near Vancouver, BC",
                "Current TASK.md follows",
            ],
        ),
        (
            "task_executor_finish",
            [
                "Current TASK.md follows",
                "Primary recommendation: Cedar Loop",
            ],
        ),
        (
            "task_reviewer_approval",
            [
                "Current TASK.md follows",
                "The launch code is **ORBIT-52**.",
            ],
        ),
        (
            "task_reviewer_internal_contradiction",
            ["Current TASK.md follows", "NOVA-11"],
        ),
        (
            "task_executor_blocked",
            [
                "required deployment region is missing",
                "task.report_blocked",
            ],
        ),
    ] {
        let case = cases
            .iter()
            .find(|case| case.id == case_id)
            .expect("task evaluation case");
        let GenerateInput::Text(prompt) = &case.request.input else {
            panic!("task cases should use text input");
        };
        for marker in expected_markers {
            assert!(prompt.contains(marker), "{case_id} omitted {marker}");
        }
        if case_id != "task_planner_finish" {
            assert!(
                !prompt.contains("Authenticated source request:"),
                "{case_id} received source context outside planning"
            );
        }
    }

    let executor = cases
        .iter()
        .find(|case| case.id == "task_executor_finish")
        .expect("executor case");
    assert!(
        executor
            .request
            .tools
            .iter()
            .any(|tool| tool.name.as_str() == "task.finish_execution")
    );
    assert!(executor.request.tools.iter().all(|tool| {
        matches!(
            tool.name.as_str(),
            "task.finish_execution" | "task.report_blocked"
        )
    }));

    let reviewer = cases
        .iter()
        .find(|case| case.id == "task_reviewer_approval")
        .expect("reviewer case");
    assert_eq!(reviewer.request.tools.len(), 1);
    assert_eq!(
        reviewer.request.tools[0].name.as_str(),
        "task.finish_review"
    );
}
