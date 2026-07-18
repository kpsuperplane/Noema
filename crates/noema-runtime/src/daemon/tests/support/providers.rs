fn estimated_test_tokens(value: &str) -> u32 {
    value.chars().count().div_ceil(3) as u32
}

#[derive(Debug)]
struct FakeCodexProvider {
    scenario: FakeCodexScenario,
    requests: Mutex<Vec<GenerateRequest>>,
    tool_capabilities: ProviderToolCapabilities,
    response_continuation: ProviderResponseContinuation,
    reject_chained_once: Mutex<bool>,
}

impl FakeCodexProvider {
    fn new(scenario: FakeCodexScenario) -> Self {
        Self {
            scenario,
            requests: Mutex::new(Vec::new()),
            tool_capabilities: ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::NoemaEnvelope,
                ..ProviderToolCapabilities::default()
            },
            response_continuation: ProviderResponseContinuation::Unsupported,
            reject_chained_once: Mutex::new(false),
        }
    }

    fn with_tool_capabilities(mut self, tool_capabilities: ProviderToolCapabilities) -> Self {
        self.tool_capabilities = tool_capabilities;
        self
    }

    fn with_response_continuation(
        mut self,
        response_continuation: ProviderResponseContinuation,
    ) -> Self {
        self.response_continuation = response_continuation;
        self
    }

    fn rejecting_first_chained_request(self) -> Self {
        *self.reject_chained_once.lock().expect("reject chained") = true;
        self
    }

    fn requests(&self) -> Vec<GenerateRequest> {
        self.requests.lock().expect("requests").clone()
    }
}

#[derive(Debug)]
struct CapturingProvider {
    capabilities: ProviderToolCapabilities,
    context_window_tokens: Option<u32>,
    fail_compaction: bool,
    fail_token_count: bool,
    enforce_context_window: bool,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl Default for CapturingProvider {
    fn default() -> Self {
        Self {
            capabilities: ProviderToolCapabilities::default(),
            context_window_tokens: None,
            fail_compaction: false,
            fail_token_count: false,
            enforce_context_window: false,
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct BlockingOnceProvider {
    started: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
}

#[derive(Debug)]
struct BlockingTaskCompletionProvider {
    completion_started: Mutex<Option<oneshot::Sender<()>>>,
    primary_started: Mutex<Option<oneshot::Sender<()>>>,
    release_completion: Mutex<Option<oneshot::Receiver<()>>>,
}

#[derive(Debug)]
struct ConcurrentTaskProvider {
    started: mpsc::UnboundedSender<String>,
}

#[derive(Debug)]
struct BlockingBackgroundGenerationProvider {
    started: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
    requests: Mutex<Vec<GenerateRequest>>,
}

#[derive(Debug, Clone, Copy)]
enum FakeCodexScenario {
    Simple,
    MultipleChoice,
    ReasoningReplay,
    InitialNameOnboarding,
    InitialNameOnboardingNoAssistant,
    TurnError,
    MultipleTaskDelegation,
    MixedTaskDelegation,
    ToolCallBeforeCommentary,
    ToolItemThenFailure,
    SearchMemoryContinuation,
    NativeSearchMemoryContinuation,
    InvalidSearchMemory,
    NativeWebFetchContinuation,
    NativeArtifactCreateLocalFileContinuation,
    LongContinuationThenFinalization,
    ProgressAuditFailsThenFinalization,
    UpdateOwnNameThenYay,
    RepeatedUpdateOwnNameContinuation,
    UpdateOwnNameThenIdentityCheck,
}

fn fake_provider(scenario: FakeCodexScenario) -> FakeCodexProvider {
    FakeCodexProvider::new(scenario)
}

fn native_fake_provider(scenario: FakeCodexScenario) -> FakeCodexProvider {
    FakeCodexProvider::new(scenario).with_tool_capabilities(ProviderToolCapabilities {
        tool_transport: ProviderToolTransport::Native,
        parallel_tool_calls: true,
        native_tool_results: true,
        schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
        ..ProviderToolCapabilities::default()
    })
}

impl FakeCodexProvider {
    fn generate_response(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, ProviderError> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| "fake-model".to_string());
        let rendered_input = request.input.render_for_token_count();
        let input = current_user_input(&request.input);
        let instructions = request.instructions.unwrap_or_default();
        let identity_context =
            latest_model_context_section(&request.input, "agent.identity").unwrap_or_default();
        let output = match self.scenario {
            FakeCodexScenario::Simple => assistant_with_no_memories("fake answer"),
            FakeCodexScenario::MultipleChoice => (
                vec![GenerateResponseItem::MultipleChoice {
                    phase: Some(AssistantTextPhase::FinalAnswer),
                    prompt: "Pick a direction".to_string(),
                    selection_mode: MultipleChoiceSelectionMode::PickOne,
                    options: vec![
                        MultipleChoiceOption {
                            id: "ship".to_string(),
                            label: "Ship it".to_string(),
                        },
                        MultipleChoiceOption {
                            id: "polish".to_string(),
                            label: "Polish first".to_string(),
                        },
                    ],
                }],
                Vec::new(),
            ),
            FakeCodexScenario::ReasoningReplay => {
                let saw_reasoning_replay = match &request.input {
                    GenerateInput::Items(items) => items.iter().any(|item| {
                        matches!(
                            item,
                            GenerateInputItem::Reasoning(reasoning)
                                if reasoning.encrypted_content == "opaque-turn-one"
                        )
                    }),
                    GenerateInput::Text(_)
                    | GenerateInput::Messages(_)
                    | GenerateInput::NativeToolResults(_) => false,
                };
                let mut response = fake_generate_response(
                    assistant_with_no_memories(if saw_reasoning_replay {
                        "saw encrypted reasoning"
                    } else {
                        "first answer"
                    }),
                    "codex",
                    model,
                );
                if !saw_reasoning_replay {
                    response.reasoning_items.push(GenerateReasoningItem {
                        id: Some("rs_fake_1".to_string()),
                        encrypted_content: Some("opaque-turn-one".to_string()),
                    });
                }
                return Ok(response);
            }
            FakeCodexScenario::InitialNameOnboarding => {
                let saw_onboarding = instructions.contains("Agent identity:")
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("Ask the user what they would like to name you.")
                    && !input.contains("Your name is");
                if saw_onboarding {
                    (
                        [
                            "hey, i’m glad to be here with you 👋",
                            "i can help you think, plan, make, untangle, and keep life moving with a little more ease",
                            "what would you like to name me?",
                        ]
                        .into_iter()
                        .map(|text| GenerateResponseItem::Text {
                            phase: None,
                            text: text.to_string(),
                        })
                        .collect(),
                        Vec::new(),
                    )
                } else {
                    assistant_with_no_memories("missing warm onboarding prompt")
                }
            }
            FakeCodexScenario::InitialNameOnboardingNoAssistant => (Vec::new(), Vec::new()),
            FakeCodexScenario::TurnError => {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "turn failed".to_string(),
                    request_id: None,
                });
            }
            FakeCodexScenario::MultipleTaskDelegation => (
                vec![
                    GenerateResponseItem::Text {
                        phase: Some(AssistantTextPhase::FinalAnswer),
                        text: "I started all three background tasks.".to_string(),
                    },
                    GenerateResponseItem::Text {
                        phase: Some(AssistantTextPhase::FinalAnswer),
                        text: "They are underway.".to_string(),
                    },
                ],
                vec![
                    task_delegate_tool_call("call_task_canada", "Research Canada", true),
                    task_delegate_tool_call("call_task_usa", "Research USA", true),
                    task_delegate_tool_call("call_task_invalid", "Invalid task", false),
                ],
            ),
            FakeCodexScenario::MixedTaskDelegation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("I could not combine delegation with another tool.")
                } else {
                    assistant_with_tools(
                        "I started the task and renamed myself.",
                        Some(AssistantTextPhase::FinalAnswer),
                        vec![
                            task_delegate_tool_call("call_task_mixed", "Mixed task", true),
                            update_own_name_tool_call("call_name_mixed", json!({"name": "Mira"})),
                        ],
                    )
                }
            }
            FakeCodexScenario::ToolCallBeforeCommentary => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("The memory check is complete.")
                } else {
                    assistant_with_tools(
                        "Checking memory.",
                        Some(AssistantTextPhase::Commentary),
                        vec![search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        )],
                    )
                }
            }
            FakeCodexScenario::ToolItemThenFailure => {
                return Err(ProviderError::PartialResponse {
                    provider: "codex".to_string(),
                    model,
                    message: "tool failed later".to_string(),
                    output: vec![search_memory_action_item(
                        "call_1",
                        json!({"arguments": {"query": "trains"}}),
                    )],
                });
            }
            FakeCodexScenario::SearchMemoryContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("I found your train memory.")
                } else if input.contains("Please remember I'm a big fan of trains") {
                    assistant_with_no_memories("fake answer")
                } else if input.contains("What do you remember about trains?") {
                    assistant_with_tools(
                        "Searching memory.",
                        None,
                        vec![search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        )],
                    )
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::NativeSearchMemoryContinuation => {
                let results = input_tool_results(&request.input);
                if results.iter().any(|result| {
                    result.call_id == "call_native_1" && result.name == "search_memory"
                }) {
                    assistant_with_no_memories("native tool result received")
                } else if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("tool result received")
                } else if input.contains("What do you remember about trains?") {
                    assistant_with_tools(
                        "Searching memory.",
                        None,
                        vec![GenerateToolCall {
                            id: Some("item_native_1".to_string()),
                            provider_call_id: Some("call_native_1".to_string()),
                            provider_name: Some("search_memory".to_string()),
                            name: "search_memory".to_string(),
                            payload: json!({"arguments": {"query": "trains"}}),
                        }],
                    )
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::InvalidSearchMemory => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("invalid request handled")
                } else {
                    assistant_with_tools(
                        "Trying memory.",
                        None,
                        vec![search_memory_tool_call(
                            "call_invalid",
                            json!({"arguments": {"query": "all", "purpose": "dump_everything"}}),
                        )],
                    )
                }
            }
            FakeCodexScenario::NativeWebFetchContinuation => {
                if input_tool_results(&request.input)
                    .iter()
                    .any(|result| result.name == "web.fetch" && result.success)
                {
                    assistant_with_no_memories("I read the fetched page.")
                } else {
                    assistant_with_tools(
                        "Fetching.",
                        None,
                        vec![GenerateToolCall {
                            id: Some("item_fetch".to_string()),
                            provider_call_id: Some("call_fetch".to_string()),
                            provider_name: Some("web.fetch".to_string()),
                            name: "web.fetch".to_string(),
                            payload: json!({"arguments": {"url": "https://example.com/page"}}),
                        }],
                    )
                }
            }
            FakeCodexScenario::NativeArtifactCreateLocalFileContinuation => {
                if input_tool_results(&request.input)
                    .iter()
                    .any(|result| result.name == "artifact.create_local_file" && result.success)
                {
                    assistant_with_no_memories("I created the two-version artifact.")
                } else {
                    assistant_with_tools(
                        "Creating the artifact.",
                        None,
                        vec![GenerateToolCall {
                            id: Some("item_artifact".to_string()),
                            provider_call_id: Some("call_artifact".to_string()),
                            provider_name: Some("artifact.create_local_file".to_string()),
                            name: "artifact.create_local_file".to_string(),
                            payload: json!({"arguments": {
                                "title": "Agent artifact smoke note",
                                "artifact_kind": "note",
                                "filename": "note.md",
                                "media_type": "text/markdown",
                                "versions": [
                                    {"title": "Draft", "content": "First version"},
                                    {"title": "Final", "content": "Second version"}
                                ]
                            }}),
                        }],
                    )
                }
            }
            FakeCodexScenario::LongContinuationThenFinalization => {
                let input_text = request.input.render_for_token_count();
                let loop_query = format!("restaurants {}", input_text.len());
                if request.tools.is_empty()
                    && !request.parallel_tool_calls
                    && instructions.contains("must stop now")
                {
                    assistant_with_no_memories(
                        "I gathered partial results and paused before the tool loop could run too long.",
                    )
                } else {
                    tool_calls_only(vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )])
                }
            }
            FakeCodexScenario::ProgressAuditFailsThenFinalization => {
                let input_text = request.input.render_for_token_count();
                let loop_query = format!("restaurants {}", input_text.len());
                if request.tools.is_empty()
                    && instructions.contains(
                        "You are auditing whether a Noema tool-continuation loop is making progress.",
                    )
                {
                    return Err(noema_providers::ProviderError::ProtocolError {
                        provider: "codex".to_string(),
                        message: "audit failed".to_string(),
                    });
                }
                if request.tools.is_empty() && instructions.contains("must stop now") {
                    assistant_with_no_memories(
                        "The progress check failed, so I am pausing with the useful work gathered so far.",
                    )
                } else {
                    tool_calls_only(vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )])
                }
            }
            FakeCodexScenario::UpdateOwnNameThenYay => {
                if input.contains("Let's rename you to Momo") {
                    tool_calls_only(vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Momo"}),
                    )])
                } else if input == "Yay" {
                    if rendered_input.contains("function_call_output")
                        && rendered_input.contains("update_own_name")
                        && rendered_input.contains("Momo")
                    {
                        assistant_with_no_memories("yay acknowledged after saved name")
                    } else {
                        tool_calls_only(vec![update_own_name_tool_call(
                            "call_name_2",
                            json!({"name": "Momo"}),
                        )])
                    }
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::RepeatedUpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    let saw_updated_identity = identity_context.contains("Agent identity:")
                        && identity_context.contains(r#"display_name: "Fred""#)
                        && !identity_context.contains("You do not have a name yet.");
                    let saw_onboarding_tasks = identity_context
                        .contains("Onboarding tasks, in priority order:")
                        && identity_context.contains("what the user wants help with first");
                    assistant_with_tools(
                        if saw_updated_identity && saw_onboarding_tasks {
                            "Fred it is. what would you like help with first?"
                        } else {
                            "same-turn identity was stale"
                        },
                        None,
                        vec![update_own_name_tool_call(
                            "call_name_2",
                            json!({"name": "Fred"}),
                        )],
                    )
                } else {
                    tool_calls_only(vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Fred"}),
                    )])
                }
            }
            FakeCodexScenario::UpdateOwnNameThenIdentityCheck => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("Mira it is.")
                } else if input.contains("Your name is Mira.") {
                    tool_calls_only(vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Mira"}),
                    )])
                } else {
                    let saw_identity = identity_context.contains("Agent identity:")
                        && identity_context.contains(r#"display_name: "Mira""#)
                        && !identity_context.contains("You do not have a name yet.");
                    assistant_with_no_memories(if saw_identity {
                        "saw stored identity"
                    } else {
                        "missing stored identity"
                    })
                }
            }
        };

        Ok(fake_generate_response(output, "codex", model))
    }
}
