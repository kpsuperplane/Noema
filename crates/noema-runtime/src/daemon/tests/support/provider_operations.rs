impl noema_providers::ProviderOperations for FakeCodexProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        self.tool_capabilities
    }

    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        self.response_continuation
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let request_number = {
                let mut requests = self.requests.lock().expect("requests");
                requests.push(request.clone());
                requests.len()
            };
            if request.options.previous_response_id.is_some() {
                let mut reject = self.reject_chained_once.lock().expect("reject chained");
                if *reject {
                    *reject = false;
                    return Err(ProviderError::ApiError {
                        status: 404,
                        message: "previous response not found".to_string(),
                        request_id: None,
                    });
                }
            }
            let mut response = self.generate_response(request)?;
            if self.response_continuation.supports_previous_response_id() {
                response.response_id = Some(format!("resp_{request_number}"));
            }
            emit_fake_stream_events(&response, Some(4), on_event);
            Ok(response)
        })
    }
}

fn emit_fake_stream_events(
    response: &GenerateResponse,
    chunk_chars: Option<usize>,
    on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
) {
    for (response_index, item) in response.responses.iter().enumerate() {
        let GenerateResponseItem::Text { text, .. } = item else {
            continue;
        };
        let chunks = chunk_chars.map_or_else(
            || vec![text.clone()],
            |size| {
                let chars = text.chars().collect::<Vec<_>>();
                chars
                    .chunks(size)
                    .map(|chunk| chunk.iter().collect())
                    .collect()
            },
        );
        for delta in chunks {
            on_event(GenerateStreamEvent::AssistantTextDelta {
                response_index,
                delta,
            });
        }
    }
    for (index, call) in response.tool_calls.iter().enumerate() {
        on_event(GenerateStreamEvent::ToolCallStarted {
            output_index: response.responses.len() + index,
            name: call.name.clone(),
        });
    }
}

impl noema_providers::ProviderOperations for CapturingProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        self.capabilities
    }

    fn context_metadata(&self, _model: Option<&str>) -> noema_providers::ProviderContextMetadata {
        noema_providers::ProviderContextMetadata {
            context_window_tokens: self.context_window_tokens,
            default_output_reserve_tokens: Some(512),
            compact_summary_target_tokens: Some(512),
        }
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        _model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            if self.fail_token_count {
                return Err(ProviderError::ProviderUnavailable {
                    provider: "test".to_string(),
                    message: "token counting failed".to_string(),
                });
            }
            let instruction_tokens = instructions.map_or(0, estimated_test_tokens);
            Ok(Some(instruction_tokens + estimated_test_tokens(input)))
        })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.clone());
            if self.enforce_context_window {
                let input = request.input.render_for_token_count();
                let input_tokens = request
                    .instructions
                    .as_deref()
                    .map_or(0, estimated_test_tokens)
                    + estimated_test_tokens(&input);
                let available = self
                    .context_window_tokens
                    .expect("enforced context window")
                    .saturating_sub(request.options.max_output_tokens.unwrap_or(512))
                    .saturating_sub(128);
                if input_tokens > available {
                    return Err(ProviderError::ApiError {
                        status: 400,
                        message: format!(
                            "exceeded context window size: Content contains {input_tokens} tokens"
                        ),
                        request_id: None,
                    });
                }
            }
            if self.fail_compaction && !request.options.require_noema_response {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "compaction failed".to_string(),
                    request_id: None,
                });
            }
            Ok(fake_generate_response(
                assistant_with_no_memories("fake answer"),
                "test",
                request.model.unwrap_or_else(|| "fake-model".to_string()),
            ))
        })
    }
}

impl noema_providers::ProviderOperations for BlockingOnceProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(started) = self.started.lock().expect("started lock").take() {
                let _ = started.send(());
            }
            let release = self
                .release
                .lock()
                .expect("release lock")
                .take()
                .expect("release receiver");
            release
                .await
                .map_err(|_| ProviderError::ProviderUnavailable {
                    provider: "test".to_string(),
                    message: "release signal dropped".to_string(),
                })?;
            Ok(fake_generate_response(
                assistant_with_no_memories("slow answer"),
                "test",
                "blocking-once".to_string(),
            ))
        })
    }
}

impl noema_providers::ProviderOperations for BlockingTaskCompletionProvider {
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let answer = if request.options.generation_priority
                == noema_providers::GenerationPriority::Background
            {
                if let Some(started) = self
                    .completion_started
                    .lock()
                    .expect("completion started lock")
                    .take()
                {
                    let _ = started.send(());
                }
                let release = self
                    .release_completion
                    .lock()
                    .expect("completion release lock")
                    .take()
                    .expect("completion release receiver");
                release
                    .await
                    .map_err(|_| ProviderError::ProviderUnavailable {
                        provider: "test".to_string(),
                        message: "completion release signal dropped".to_string(),
                    })?;
                "completion answer"
            } else {
                if let Some(started) = self
                    .primary_started
                    .lock()
                    .expect("primary started lock")
                    .take()
                {
                    let _ = started.send(());
                }
                "foreground answer"
            };
            Ok(fake_generate_response(
                assistant_with_no_memories(answer),
                "test",
                "blocking-task-completion".to_string(),
            ))
        })
    }
}

impl noema_providers::ProviderOperations for ConcurrentTaskProvider {
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let run_id = request
                .conversation_id
                .as_deref()
                .and_then(|conversation_id| conversation_id.strip_prefix("task_run:"))
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "unknown".to_string());
            let _ = self.started.send(run_id);
            std::future::pending::<Result<GenerateResponse, ProviderError>>().await
        })
    }
}

impl noema_providers::ProviderOperations for BlockingBackgroundGenerationProvider {
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
        }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let call_index = {
                let mut requests = self.requests.lock().expect("requests");
                requests.push(request.clone());
                requests.len()
            };
            match call_index {
                1 => {
                    self.started
                        .lock()
                        .expect("started")
                        .take()
                        .expect("first call")
                        .send(())
                        .expect("signal first call");
                    let release = self
                        .release
                        .lock()
                        .expect("release")
                        .take()
                        .expect("first release");
                    release.await.expect("release old generation");
                    Ok(fake_generate_response(
                        assistant_with_no_memories("working"),
                        "old-local",
                        request.model.unwrap_or_else(|| "old-model".to_string()),
                    ))
                }
                2 => Ok(fake_generate_response(
                    tool_calls_only(vec![GenerateToolCall {
                        id: Some("call_submit_result".to_string()),
                        provider_call_id: None,
                        provider_name: None,
                        name: "task.submit_result".to_string(),
                        payload: json!({
                            "summary": "old generation result",
                            "result_markdown": "done",
                            "criteria": [],
                        }),
                    }]),
                    "old-local",
                    request.model.unwrap_or_else(|| "old-model".to_string()),
                )),
                _ => panic!("unexpected provider call {call_index}"),
            }
        })
    }
}
