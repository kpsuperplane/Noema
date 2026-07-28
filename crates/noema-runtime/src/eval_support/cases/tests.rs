use noema_providers::{GenerateInput, NoemaToolChoice};

use super::evaluation_cases;

#[test]
fn onboarding_case_requires_the_name_tool_for_an_unnamed_agent() {
    let cases = evaluation_cases("local-model").expect("cases");
    assert_eq!(cases.len(), 15, "qualification request contract changed");
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
                "Authenticated source request:\nFind hikes near Vancouver, BC",
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
            "task_reviewer_internal_contradiction",
            ["Executor submission:", "NOVA-11"],
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
            assert!(prompt.contains("Default to simple"));
            assert!(prompt.contains("Keep request_markdown to a concise restatement"));
            assert!(prompt.contains("at most two short execution phases"));
            assert!(prompt.contains("stop condition explicit"));
        }
    }

    let executor = cases
        .iter()
        .find(|case| case.id == "task_executor_submission")
        .expect("executor case");
    let GenerateInput::Text(executor_prompt) = &executor.request.input else {
        panic!("executor case should use text input");
    };
    assert!(executor_prompt.contains("one discovery batch"));
    assert!(executor_prompt.contains("roughly 180 words"));
    assert!(executor_prompt.contains("reuse relevant work"));
    assert!(executor_prompt.contains("complete replacement deliverable"));
    assert!(executor_prompt.contains("never submit only a patch"));

    let reviewer = cases
        .iter()
        .find(|case| case.id == "task_reviewer_approval")
        .expect("reviewer case");
    let GenerateInput::Text(reviewer_prompt) = &reviewer.request.input else {
        panic!("reviewer case should use text input");
    };
    assert!(reviewer_prompt.contains("complete authorized evidence"));
    assert!(reviewer_prompt.contains("prior submissions are not inherited"));
    assert!(reviewer_prompt.contains("assertion is not a substitute"));
    assert!(reviewer_prompt.contains("background knowledge"));
}
