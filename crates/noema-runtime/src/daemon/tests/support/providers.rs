fn estimated_test_tokens(value: &str) -> u32 {
    value.chars().count().div_ceil(3) as u32
}

#[derive(Debug, Clone)]
struct FakeCodexProvider {
    scenario: FakeCodexScenario,
}

#[derive(Debug)]
struct RecordingFakeProvider {
    provider_kind: String,
    inner: FakeCodexProvider,
    requests: Mutex<Vec<GenerateRequest>>,
    tool_capabilities: ProviderToolCapabilities,
    response_continuation: ProviderResponseContinuation,
    reject_chained_once: Mutex<bool>,
}

impl RecordingFakeProvider {
    fn new(provider_kind: &str, scenario: FakeCodexScenario) -> Self {
        Self {
            provider_kind: provider_kind.to_string(),
            inner: FakeCodexProvider::new(scenario),
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
    requests: Mutex<Vec<GenerateRequest>>,
}

impl Default for CapturingProvider {
    fn default() -> Self {
        Self {
            capabilities: ProviderToolCapabilities::default(),
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct MetadataCapturingProvider {
    context_window_tokens: u32,
    fail_compaction: bool,
    fail_token_count: bool,
    enforce_context_window: bool,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl Default for MetadataCapturingProvider {
    fn default() -> Self {
        Self {
            context_window_tokens: 5_500,
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
    RestartContext,
    IdentityPromptCheck,
    PromptPhaseContract,
    PromptMarkdownContract,
    InitialNameOnboarding,
    InitialNameOnboardingNoAssistant,
    TurnError,
    ToolItem,
    MultipleTaskDelegation,
    MixedTaskDelegation,
    ToolCallBeforeCommentary,
    ToolItemThenFailure,
    InvalidSearchMemory,
    SearchMemoryContinuation,
    NativeSearchMemoryContinuation,
    NativeWebSearch,
    NativeWebSearchContinuation,
    NativeWebFetchContinuation,
    NativeArtifactCreateLocalFileContinuation,
    ChainedSearchMemoryContinuation,
    MemoryContextQuestion,
    LongContinuationThenFinalization,
    ProgressAuditFailsThenFinalization,
    SearchMemoryProfileContinuation,
    UpdateOwnNameContinuation,
    UpdateOwnNameThenYay,
    RepeatedUpdateOwnNameContinuation,
    AmbiguousUpdateOwnName,
    UpdateOwnNameThenIdentityCheck,
}

fn fake_provider(scenario: FakeCodexScenario) -> FakeCodexProvider {
    FakeCodexProvider::new(scenario)
}

impl FakeCodexProvider {
    fn new(scenario: FakeCodexScenario) -> Self {
        Self { scenario }
    }

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
            FakeCodexScenario::MultipleChoice => vec![GenerateOutputItem::MultipleChoice {
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
            FakeCodexScenario::ReasoningReplay => {
                let saw_reasoning_replay = match &request.input {
                    GenerateInput::Items(items) => items.iter().any(|item| {
                        matches!(
                            item,
                            GenerateInputItem::Reasoning(reasoning)
                                if reasoning.encrypted_content == "opaque-turn-one"
                        )
                    }),
                    _ => false,
                };
                let mut response = fake_generate_response(
                    vec![GenerateOutputItem::AssistantText {
                        phase: None,
                        text: if saw_reasoning_replay {
                            "saw encrypted reasoning"
                        } else {
                            "first answer"
                        }
                        .to_string(),
                    }],
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
            FakeCodexScenario::RestartContext => {
                let saw_context = rendered_input.contains("first durable question")
                    && rendered_input.contains("fake answer")
                    && input.contains("second durable question")
                    && !instructions.contains("Recent durable transcript")
                    && !instructions.contains("first durable question");
                assistant_with_no_memories(if saw_context {
                    "saw durable context"
                } else {
                    "fake answer"
                })
            }
            FakeCodexScenario::IdentityPromptCheck => {
                let saw_identity = identity_context.contains("Agent identity:")
                    && identity_context.contains(r#"agent_id: "agent:primary""#)
                    && identity_context.contains("display_name: null")
                    && identity_context.contains("Onboarding prompt:")
                    && identity_context.contains("update_own_name");
                assistant_with_no_memories(if saw_identity {
                    "saw unnamed identity"
                } else {
                    "missing unnamed identity"
                })
            }
            FakeCodexScenario::PromptPhaseContract => {
                let saw_phase_contract = instructions.contains(r#""phase":"commentary""#)
                    && instructions.contains(r#""phase":"final_answer""#)
                    && instructions.contains("Use phase \"commentary\" for text that explains what you are about to do before a tool result is available.")
                    && instructions.contains("Use phase \"final_answer\" only for the terminal answer after required tool results are available.");
                assistant_with_no_memories(if saw_phase_contract {
                    "saw phase contract"
                } else {
                    "missing phase contract"
                })
            }
            FakeCodexScenario::PromptMarkdownContract => {
                let saw_markdown_contract = instructions.contains(
                    "User-visible assistant text may use Markdown when it makes the answer clearer.",
                ) && instructions.contains(
                    "Keep Markdown inside responses[].text; the outer response must remain strict JSON.",
                );
                assistant_with_no_memories(if saw_markdown_contract {
                    "saw markdown contract"
                } else {
                    "missing markdown contract"
                })
            }
            FakeCodexScenario::InitialNameOnboarding => {
                let saw_onboarding = instructions.contains("Agent identity:")
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("Ask the user what they would like to name you.")
                    && instructions.contains("warm and welcoming")
                    && instructions.contains("energy")
                    && instructions.contains("Noema personal agent")
                    && instructions.contains("keep life moving with a little more ease")
                    && instructions.contains("what they would like to name you")
                    && instructions.contains("think, plan, make, untangle")
                    && instructions
                        .contains("Split the introduction into three separate text responses")
                    && instructions.contains("Always include exactly three text responses")
                    && !input.contains("Your name is");
                if saw_onboarding {
                    assistant_items_with_no_memories(&[
                        "hey, i’m glad to be here with you 👋",
                        "i can help you think, plan, make, untangle, and keep life moving with a little more ease",
                        "what would you like to name me?",
                    ])
                } else {
                    assistant_with_no_memories("missing warm onboarding prompt")
                }
            }
            FakeCodexScenario::InitialNameOnboardingNoAssistant => Vec::new(),
            FakeCodexScenario::TurnError => {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "turn failed".to_string(),
                    request_id: None,
                });
            }
            FakeCodexScenario::ToolItem => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("tool result received")
                } else {
                    vec![
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "fake answer".to_string(),
                        },
                    ]
                }
            }
            FakeCodexScenario::MultipleTaskDelegation => vec![
                GenerateOutputItem::AssistantText {
                    phase: Some(AssistantTextPhase::FinalAnswer),
                    text: "I started all three background tasks.".to_string(),
                },
                GenerateOutputItem::AssistantText {
                    phase: Some(AssistantTextPhase::FinalAnswer),
                    text: "They are underway.".to_string(),
                },
                task_delegate_tool_call("call_task_canada", "Research Canada", true),
                task_delegate_tool_call("call_task_usa", "Research USA", true),
                task_delegate_tool_call("call_task_invalid", "Invalid task", false),
            ],
            FakeCodexScenario::MixedTaskDelegation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("I could not combine delegation with another tool.")
                } else {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: Some(AssistantTextPhase::FinalAnswer),
                            text: "I started the task and renamed myself.".to_string(),
                        },
                        task_delegate_tool_call("call_task_mixed", "Mixed task", true),
                        update_own_name_tool_call("call_name_mixed", json!({"name": "Mira"})),
                    ]
                }
            }
            FakeCodexScenario::ToolCallBeforeCommentary => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("The memory check is complete.")
                } else {
                    vec![
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::AssistantText {
                            phase: Some(noema_providers::AssistantTextPhase::Commentary),
                            text: "Checking memory.".to_string(),
                        },
                    ]
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
            FakeCodexScenario::InvalidSearchMemory => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("invalid tool result received")
                } else {
                    vec![search_memory_tool_call(
                        "call_bad",
                        json!({"arguments": {"query": "trains", "purpose": "dump_everything"}}),
                    )]
                }
            }
            FakeCodexScenario::SearchMemoryContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("I found your train memory.")
                } else if input.contains("Please remember I'm a big fan of trains") {
                    assistant_with_no_memories("fake answer")
                } else if input.contains("What do you remember about trains?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                    ]
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
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Searching memory.".to_string(),
                        },
                        GenerateOutputItem::ToolCall {
                            id: Some("item_native_1".to_string()),
                            provider_call_id: Some("call_native_1".to_string()),
                            provider_name: Some("search_memory".to_string()),
                            name: "search_memory".to_string(),
                            payload: json!({"arguments": {"query": "trains"}}),
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::NativeWebSearch => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("web search result received")
                } else {
                    vec![web_search_tool_call(
                        "call_web_1",
                        json!({
                            "query": "rust language",
                            "reason": "answer the user's request",
                            "max_results": 3
                        }),
                    )]
                }
            }
            FakeCodexScenario::NativeWebSearchContinuation => {
                let results = input_tool_results(&request.input);
                if results.iter().any(|result| result.name == "web.search") {
                    assistant_with_no_memories("I found a current web result.")
                } else if !results.is_empty() {
                    assistant_with_no_memories("wrong web search tool result")
                } else {
                    vec![web_search_tool_call(
                        "call_web_1",
                        json!({
                            "query": "rust language",
                            "reason": "answer the current question",
                            "max_results": 3
                        }),
                    )]
                }
            }
            FakeCodexScenario::NativeWebFetchContinuation => {
                let results = input_tool_results(&request.input);
                if results.iter().any(|result| result.name == "web.fetch") {
                    assistant_with_no_memories("I read the fetched page.")
                } else if !results.is_empty() {
                    assistant_with_no_memories("wrong web fetch tool result")
                } else {
                    vec![web_fetch_tool_call(
                        "call_fetch_1",
                        json!({
                            "url": "https://example.com/page",
                            "reason": "answer the current question",
                            "max_chars": 5000
                        }),
                    )]
                }
            }
            FakeCodexScenario::NativeArtifactCreateLocalFileContinuation => {
                let results = input_tool_results(&request.input);
                if results.iter().any(|result| {
                    result.name == "artifact.create_local_file"
                        && result.success
                        && result.payload["current_version_index"] == 2
                }) {
                    assistant_with_no_memories("I created the two-version artifact.")
                } else if !results.is_empty() {
                    assistant_with_no_memories("wrong artifact tool result")
                } else {
                    vec![artifact_create_local_file_tool_call(
                        "call_artifact_1",
                        json!({
                            "title": "Agent artifact smoke note",
                            "description": "Created by the agent artifact tool test.",
                            "artifact_kind": "document",
                            "filename": "agent-artifact-smoke-note.md",
                            "media_type": "text/markdown",
                            "versions": [
                                {
                                    "title": "Draft",
                                    "content": "# Agent artifact smoke note\n\nVersion one."
                                },
                                {
                                    "title": "Revision",
                                    "content": "# Agent artifact smoke note\n\nVersion two."
                                }
                            ]
                        }),
                    )]
                }
            }
            FakeCodexScenario::ChainedSearchMemoryContinuation => {
                let history = request.input.render_for_token_count();
                if history.contains("call_2") {
                    assistant_with_no_memories("I checked both memory topics.")
                } else if history.contains("call_1") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "I need one more memory check.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_2",
                            json!({"arguments": {"query": "planes"}}),
                        ),
                    ]
                } else if input.contains("Check memory twice before answering.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Checking memory first.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MemoryContextQuestion => {
                if input.contains("start memory context test") {
                    assistant_items_with_no_memories(&[
                        "what are some topics you find interesting?",
                        "short answers are fine too",
                    ])
                } else {
                    assistant_with_no_memories("got it")
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
                    vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )]
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
                    vec![search_memory_tool_call(
                        "call_loop",
                        json!({"arguments": {"query": loop_query}}),
                    )]
                }
            }
            FakeCodexScenario::SearchMemoryProfileContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![GenerateOutputItem::AssistantText {
                        phase: None,
                        text: "I remember that you like planes.".to_string(),
                    }]
                } else if input.contains("What memories do you have of me?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_profile",
                            json!({"arguments": {
                                "scope_ids": ["human:local"],
                                "query": "",
                                "purpose": "answer_human_question",
                                "limit": 8
                            }}),
                        ),
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::UpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    let expected_name = if identity_context.contains(r#"display_name: "Fred""#) {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    let display_name_marker = format!(r#"display_name: "{expected_name}""#);
                    let saw_updated_identity = identity_context.contains("Agent identity:")
                        && identity_context.contains(&display_name_marker)
                        && !identity_context.contains("You do not have a name yet.");
                    let saw_onboarding_tasks = identity_context
                        .contains("Onboarding tasks, in priority order:")
                        && identity_context.contains("what the user wants help with first");
                    if saw_updated_identity && saw_onboarding_tasks {
                        let reply =
                            format!("{expected_name} it is. what would you like help with first?");
                        assistant_with_no_memories(&reply)
                    } else if saw_updated_identity {
                        let reply = format!("{expected_name} it is.");
                        assistant_with_no_memories(&reply)
                    } else {
                        assistant_with_no_memories("same-turn identity was stale")
                    }
                } else {
                    let name = if input.contains("Fred") {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": name}),
                    )]
                }
            }
            FakeCodexScenario::UpdateOwnNameThenYay => {
                if input.contains("Let's rename you to Momo") {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Momo"}),
                    )]
                } else if input == "Yay" {
                    if rendered_input.contains("function_call_output")
                        && rendered_input.contains("update_own_name")
                        && rendered_input.contains("Momo")
                    {
                        assistant_with_no_memories("yay acknowledged after saved name")
                    } else {
                        vec![update_own_name_tool_call(
                            "call_name_2",
                            json!({"name": "Momo"}),
                        )]
                    }
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::RepeatedUpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![
                        update_own_name_tool_call("call_name_2", json!({"name": "Fred"})),
                        GenerateOutputItem::AssistantText {
                            phase: None,
                            text: "Fred it is.".to_string(),
                        },
                    ]
                } else {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Fred"}),
                    )]
                }
            }
            FakeCodexScenario::AmbiguousUpdateOwnName => {
                assistant_with_no_memories("Please confirm what you'd like to call me.")
            }
            FakeCodexScenario::UpdateOwnNameThenIdentityCheck => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("Mira it is.")
                } else if input.contains("Your name is Mira.") {
                    vec![update_own_name_tool_call(
                        "call_name_1",
                        json!({"name": "Mira"}),
                    )]
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
