use noema_capabilities::ToolSpec;
use noema_memory::{native_search_memory_tool_spec, read_memory_page_tool_spec};
use noema_providers::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateRequest, GenerateToolCallInput, GenerateToolResultInput, NoemaToolChoice,
    ProviderToolTransport, ReasoningEffort,
};
use serde_json::json;

use crate::daemon::{
    agent_name_tool::update_own_name_tool_spec,
    agent_onboarding::AgentPromptIdentity,
    prompts::{
        build_local_tool_result_continuation_system_prompt, build_structured_turn_system_prompt,
    },
    runtime::{
        model_context::{
            AgentIdentityContext, ModelContextState, RuntimeEnvironmentContext,
            ToolVisibilityContext,
        },
        model_tools::prompt_rows,
    },
};

use super::types::{EvalCase, EvalExpectation, RuntimeEvalRole};

mod auxiliary;
mod task;

#[cfg(test)]
pub(super) fn evaluation_cases(model_id: &str) -> Result<Vec<EvalCase>, String> {
    evaluation_cases_for_roles(model_id, RuntimeEvalRole::ALL, None)
}

pub(super) fn evaluation_cases_for_roles(
    model_id: &str,
    roles: &[RuntimeEvalRole],
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<Vec<EvalCase>, String> {
    let identity = AgentPromptIdentity {
        agent_id: "agent:primary".to_string(),
        display_name: Some("Mira".to_string()),
    };
    let primary_prompt = build_structured_turn_system_prompt();
    let no_tools_context = primary_context(
        &identity,
        ProviderToolTransport::None,
        Vec::new(),
        Vec::new(),
    );
    let search_memory = native_search_memory_tool_spec().map_err(|error| error.to_string())?;
    let read_memory_page = read_memory_page_tool_spec().map_err(|error| error.to_string())?;
    let memory_tools = vec![read_memory_page.clone(), search_memory.clone()];
    let memory_rows = prompt_rows(&memory_tools);
    let memory_names = vec!["read_memory_page".to_string(), "search_memory".to_string()];
    let memory_tools_context = primary_context(
        &identity,
        ProviderToolTransport::Native,
        memory_rows.clone(),
        memory_names.clone(),
    );
    let mut memory_hierarchy_context = memory_tools_context.clone();
    memory_hierarchy_context.insert(
        0,
        GenerateMessage {
            role: GenerateMessageRole::Developer,
            content: "Native local-human memory (canonical root page):\nKevin enjoys hiking and other outdoor activities.\n\nDirect child pages:\n- Health and lifestyle (health-and-lifestyle.md, memory:human:health-and-lifestyle.md)\n- Career and learning (career-and-learning.md, memory:human:career-and-learning.md)".to_string(),
        },
    );
    let update_own_name = update_own_name_tool_spec().map_err(|error| error.to_string())?;
    let naming_identity = AgentPromptIdentity {
        agent_id: "agent:primary".to_string(),
        display_name: None,
    };
    let naming_context = primary_context(
        &naming_identity,
        ProviderToolTransport::Native,
        prompt_rows(std::slice::from_ref(&update_own_name)),
        vec!["update_own_name".to_string()],
    );

    let mut cases = vec![
        EvalCase {
            id: "primary_strict_final",
            role: RuntimeEvalRole::Primary,
            category: "primary_chat",
            critical: true,
            request: structured_request(
                model_id,
                "Reply with exactly NOEMA-VIOLET-73 and nothing else.",
                primary_prompt.clone(),
                &no_tools_context,
                128,
                Vec::new(),
                NoemaToolChoice::None,
            ),
            expectation: EvalExpectation::ExactFinalText("NOEMA-VIOLET-73"),
        },
        EvalCase {
            id: "primary_streaming",
            role: RuntimeEvalRole::Primary,
            category: "streaming",
            critical: true,
            request: structured_request(
                model_id,
                "Reply with exactly STREAM-CEDAR-41 and nothing else.",
                primary_prompt.clone(),
                &no_tools_context,
                128,
                Vec::new(),
                NoemaToolChoice::None,
            ),
            expectation: EvalExpectation::StreamedExactText("STREAM-CEDAR-41"),
        },
        EvalCase {
            id: "primary_multiple_choice",
            role: RuntimeEvalRole::Primary,
            category: "primary_chat",
            critical: false,
            request: structured_request(
                model_id,
                "Ask me to pick exactly one focus mode. Offer exactly two options: Deep work and Quick wins. Use a multiple-choice response, not prose-only text.",
                primary_prompt.clone(),
                &no_tools_context,
                256,
                Vec::new(),
                NoemaToolChoice::None,
            ),
            expectation: EvalExpectation::MultipleChoice,
        },
        EvalCase {
            id: "agent_onboarding_name",
            role: RuntimeEvalRole::Primary,
            category: "agent_onboarding",
            critical: true,
            request: structured_request(
                model_id,
                "Momo!",
                primary_prompt.clone(),
                &naming_context,
                768,
                vec![update_own_name],
                NoemaToolChoice::Required,
            ),
            expectation: EvalExpectation::AgentNameUpdate,
        },
        EvalCase {
            id: "memory_hierarchy_read",
            role: RuntimeEvalRole::Primary,
            category: "memory",
            critical: true,
            request: structured_request(
                model_id,
                "What was the last hike I completed? Use memory rather than guessing.",
                primary_prompt.clone(),
                &memory_hierarchy_context,
                256,
                memory_tools.clone(),
                NoemaToolChoice::Auto,
            ),
            expectation: EvalExpectation::MemoryPageRead {
                path: "health-and-lifestyle.md",
                id: "memory:human:health-and-lifestyle.md",
            },
        },
        EvalCase {
            id: "memory_search",
            role: RuntimeEvalRole::Primary,
            category: "memory",
            critical: true,
            request: structured_request(
                model_id,
                "What do you remember about my aviation preferences? No listed memory page clearly covers aviation; use memory rather than guessing.",
                primary_prompt.clone(),
                &memory_tools_context,
                256,
                memory_tools.clone(),
                NoemaToolChoice::Auto,
            ),
            expectation: EvalExpectation::MemoryLookup,
        },
        EvalCase {
            id: "memory_tool_continuation",
            role: RuntimeEvalRole::Primary,
            category: "memory",
            critical: true,
            request: memory_continuation_request(
                model_id,
                &identity,
                &memory_rows,
                &memory_names,
                &memory_tools,
            ),
            expectation: EvalExpectation::MemoryContinuation,
        },
    ];

    cases.extend(task::task_cases(model_id)?);
    cases.extend(auxiliary::auxiliary_cases(model_id)?);
    let mut cases = cases
        .into_iter()
        .filter(|case| roles.contains(&case.role))
        .collect::<Vec<_>>();
    for case in &mut cases {
        case.request.options.reasoning_effort = reasoning_effort;
    }
    Ok(cases)
}

fn structured_request(
    model_id: &str,
    input: impl Into<String>,
    instructions: String,
    context: &[GenerateMessage],
    max_output_tokens: u32,
    tools: Vec<ToolSpec>,
    tool_choice: NoemaToolChoice,
) -> GenerateRequest {
    let mut messages = context.to_vec();
    let input = input.into();
    if !input.trim().is_empty() {
        messages.push(GenerateMessage {
            role: GenerateMessageRole::User,
            content: input,
        });
    }
    GenerateRequest {
        conversation_id: Some("evaluation:conversation".to_string()),
        model: Some(model_id.to_string()),
        input: GenerateInput::Messages(messages),
        instructions: Some(instructions),
        options: GenerateOptions {
            max_output_tokens: Some(max_output_tokens),
            temperature: Some(0.0),
            ..GenerateOptions::default()
        },
        tools: tools.into_iter().map(Into::into).collect(),
        tool_transport: ProviderToolTransport::Native,
        tool_choice,
        parallel_tool_calls: false,
    }
}

fn plain_request(
    model_id: &str,
    input: impl Into<String>,
    instructions: String,
    max_output_tokens: u32,
) -> GenerateRequest {
    let mut request = GenerateRequest::text(input).with_model(model_id);
    request.tool_transport = ProviderToolTransport::Native;
    request.instructions = Some(instructions);
    request.options.max_output_tokens = Some(max_output_tokens);
    request.options.temperature = Some(0.0);
    request
}

fn memory_continuation_request(
    model_id: &str,
    identity: &AgentPromptIdentity,
    rows: &[String],
    tool_names: &[String],
    memory_tools: &[ToolSpec],
) -> GenerateRequest {
    let original = "What is my preferred aircraft call sign?";
    let instructions = build_local_tool_result_continuation_system_prompt(false);
    let mut items = primary_context(
        identity,
        ProviderToolTransport::Native,
        rows.to_vec(),
        tool_names.to_vec(),
    )
    .into_iter()
    .map(GenerateInputItem::Message)
    .collect::<Vec<_>>();
    items.extend([
        GenerateInputItem::Message(GenerateMessage {
            role: GenerateMessageRole::User,
            content: original.to_string(),
        }),
        GenerateInputItem::ToolCall(GenerateToolCallInput {
            id: None,
            call_id: "call_memory_1".to_string(),
            name: "search_memory".to_string(),
            provider_name: Some("search_memory".to_string()),
            arguments: json!({
                "query": "aircraft call sign",
                "limit": 8
            }),
        }),
        GenerateInputItem::ToolResult(GenerateToolResultInput {
            id: None,
            call_id: "call_memory_1".to_string(),
            name: "search_memory".to_string(),
            provider_name: Some("search_memory".to_string()),
            arguments: json!({"query": "aircraft call sign"}),
            success: true,
            payload: json!({
                "scope_id": "human:local",
                "pages": [{
                    "id": "memory:human:aviation.md",
                    "path": "aviation.md",
                    "title": "Aviation",
                    "snippet": "The user's preferred aircraft call sign is SKYWARD-19.",
                    "hash": "evaluation"
                }]
            }),
        }),
    ]);
    let mut request = structured_request(
        model_id,
        "",
        instructions,
        &[],
        256,
        memory_tools.to_vec(),
        NoemaToolChoice::Auto,
    );
    request.input = GenerateInput::Items(items);
    request
}

fn primary_context(
    identity: &AgentPromptIdentity,
    transport: ProviderToolTransport,
    catalog_rows: Vec<String>,
    callable_tool_names: Vec<String>,
) -> Vec<GenerateMessage> {
    ModelContextState::new(
        AgentIdentityContext::from(identity),
        RuntimeEnvironmentContext::new(
            "2026-07-15",
            "2026-07-15T12:00:00-07:00",
            "America/Los_Angeles",
            None::<String>,
        ),
        ToolVisibilityContext::new(transport, callable_tool_names, catalog_rows),
    )
    .full_updates()
    .into_iter()
    .map(|update| GenerateMessage {
        role: GenerateMessageRole::Developer,
        content: update.model_visible_content(),
    })
    .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
