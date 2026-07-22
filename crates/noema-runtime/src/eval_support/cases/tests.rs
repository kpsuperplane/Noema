use noema_providers::{GenerateInput, NoemaToolChoice};

use super::evaluation_cases;

#[test]
fn onboarding_case_requires_the_name_tool_for_an_unnamed_agent() {
    let cases = evaluation_cases("local-model").expect("cases");
    assert_eq!(cases.len(), 13, "qualification request contract changed");
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
fn task_cases_render_the_production_work_context_contract() {
    let cases = evaluation_cases("local-model").expect("cases");
    for (case_id, expected_markers) in [
        (
            "task_planner_contract",
            [
                "Authenticated source request:\nReturn the launch code",
                "task.submit_plan",
            ],
        ),
        (
            "task_executor_submission",
            ["Complexity: simple", "Workspace snapshot:"],
        ),
        (
            "task_reviewer_approval",
            ["Executor submission:", "criterion:alpha:"],
        ),
        (
            "task_executor_blocked",
            ["criterion_id=criterion:region", "task.report_blocked"],
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
        if case_id != "task_planner_contract" {
            assert!(
                !prompt.contains("Authenticated source request:"),
                "{case_id} received source context outside planning"
            );
        } else {
            assert!(prompt.contains("one primary recommendation and at most two alternatives"));
        }
    }
}
