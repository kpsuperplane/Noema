impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    async fn reconcile_model_context_plan(
        &self,
        provider: &dyn noema_providers::ProviderOperations,
        conversation_id: &str,
        turn_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
        state: &ModelContextState,
        current_input: &str,
        planned_context: &mut super::prompt_context::PlannedPromptContext,
    ) -> Result<usize, RuntimeError> {
        let mut appended_update_count = 0usize;
        loop {
            let updates = sync_model_context(ModelContextSyncRequest {
                store: &self.store,
                conversation_id,
                turn_id,
                provider_kind,
                model_profile,
                state,
            })
            .await?;
            if updates.is_empty() {
                return Ok(appended_update_count);
            }
            appended_update_count = appended_update_count.saturating_add(updates.len());
            let memory_root_context = self.native_memory_context();
            *planned_context = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &self.store,
                    provider,
                    conversation_id,
                    provider_kind,
                    model_profile,
                    current_input,
                    memory_root_context: memory_root_context.as_deref(),
                },
            )
            .await?;
        }
    }

    fn schedule_background_context_compaction(
        &self,
        schedule: BackgroundContextCompactionSchedule,
    ) {
        let store = self.store.clone();
        let actor = self.clone_for_background();
        self.tasks.spawn(async move {
            let BackgroundContextCompactionSchedule {
                conversation_id,
                provider_kind,
                model_profile,
                reasoning_effort,
                provider_route,
                next_turn_index,
            } = schedule;
            let provider = provider_route.operations();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            match store.next_conversation_turn_index(&conversation_id).await {
                Ok(current_next_turn_index) if current_next_turn_index == next_turn_index => {}
                Ok(_) | Err(_) => return,
            }
            let plan = super::prompt_context::plan_prompt_context(
                super::prompt_context::PromptPlanRequest {
                    store: &store,
                    provider,
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
                    current_input: "",
                    memory_root_context: None,
                },
            )
            .await;
            let Ok(plan) = plan else {
                return;
            };
            if !super::context_compaction::should_compact_background(&plan) {
                return;
            }
            let result = super::context_compaction::compact_context(
                super::context_compaction::CompactionRequest {
                    store: &store,
                    provider,
                    conversation_id: &conversation_id,
                    provider_kind: &provider_kind,
                    model_profile: model_profile.as_deref(),
                    reasoning_effort,
                    budget: plan.budget,
                    mode: super::context_compaction::CompactionMode::Background,
                },
            )
            .await;
            match result {
                Ok(_) => {
                    if let Ok(event) = super::context_compaction::persist_context_compaction_notice(
                        &store,
                        &conversation_id,
                        None,
                        None,
                    )
                    .await
                    {
                        actor.runtime_events.publish_conversation(
                            crate::daemon::ConversationRuntimeEvent::Turn {
                                client_message_id: None,
                                event: Box::new(event),
                            },
                        );
                    }
                    actor.schedule_background_native_memory_update(conversation_id.clone());
                }
                Err(error) => {
                    let _ = super::context_compaction::record_failed_background_compaction(
                        &store,
                        &conversation_id,
                        &provider_kind,
                        model_profile.as_deref(),
                        &error,
                    )
                    .await;
                }
            }
        });
    }

    pub(super) fn schedule_background_native_memory_update(&self, conversation_id: String) -> bool {
        let Some(native_memory) = self.native_memory.clone() else { return false };
        let active = Arc::clone(&self.native_memory_update_active);
        if active.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.runtime_events
            .publish_memory(crate::daemon::MemoryRuntimeEvent::Changed);
        let actor = self.clone_for_background();
        self.tasks.spawn(async move {
            let result = actor.run_native_memory_update(&native_memory, &conversation_id).await;
            active.store(false, Ordering::Release);
            if let Ok(mut last_error) = actor.native_memory_update_error.write() {
                *last_error = result.as_ref().err().cloned();
            }
            actor
                .runtime_events
                .publish_memory(crate::daemon::MemoryRuntimeEvent::Changed);
            if let Err(error) = result {
                actor.system_errors.try_append(SystemErrorEvent::new(
                    "native_memory_update_failed",
                    "Native memory update failed",
                ).with_error_chain([error]));
            }
        });
        true
    }

    async fn run_native_memory_update(
        &self,
        native_memory: &noema_memory::NativeMemory,
        conversation_id: &str,
    ) -> Result<(), String> {
        let primary = self.store.primary_conversation_for_human("human:local").await.map_err(|error| error.to_string())?;
        if primary.as_ref().map(|conversation| conversation.conversation_id.as_str()) != Some(conversation_id) {
            return Err("native memory updates require the local primary conversation".to_string());
        }
        let checkpoint = native_memory.state().map_err(|error| error.to_string())?;
        let cursor = if checkpoint.conversation_id.as_deref() == Some(conversation_id) {
            checkpoint.last_consolidated_sequence
        } else {
            0
        };
        let captured = self.store.capture_memory_source_range(
            conversation_id,
            cursor,
        ).await.map_err(|error| error.to_string())?;
        if captured.items.is_empty() {
            return Ok(());
        }
        let route = self.resolve_memory_provider().await.map_err(|error| error.to_string())?;
        let selection = route.selection().clone();
        let provider = route.operations();
        let context_budget = provider.context_metadata(selection.model_profile.as_deref())
            .context_window_tokens
            .unwrap_or(8_000)
            .saturating_sub(2_048)
            .saturating_mul(3)
            .max(1) as usize;
        let items = captured.items;
        let mut offset = 0;
        while offset < items.len() {
            let canonical_pages = native_memory.list_pages().map_err(|error| error.to_string())?;
            let mut allowed_sources = canonical_pages
                .iter()
                .flat_map(|page| page.sources.iter().cloned())
                .collect::<std::collections::HashSet<_>>();
            let mut editable = HashSet::from([noema_memory::ROOT_PAGE_PATH.to_string()]);
            let minimal = memory_prompt_catalog(&canonical_pages, &editable)?;
            let minimal_chars = minimal.chars().count();
            if minimal_chars >= context_budget {
                return Err(format!("memory page catalog exceeds the model context budget ({minimal_chars} >= {context_budget} characters)"));
            }
            let all_editable = canonical_pages
                .iter()
                .map(|page| page.path.clone())
                .collect::<HashSet<_>>();
            let full = memory_prompt_catalog(&canonical_pages, &all_editable)?;
            let full_chars = full.chars().count();
            let mut end = offset;
            let mut chunk_chars = if full_chars < context_budget {
                full_chars
            } else {
                minimal_chars.saturating_add(context_budget.saturating_sub(minimal_chars) / 3)
            };
            while end < items.len() {
                let item_chars = items[end].content_text.as_deref().map_or(0, |text| text.chars().count()).saturating_add(80);
                if end > offset && chunk_chars.saturating_add(item_chars) > context_budget {
                    break;
                }
                if end == offset && chunk_chars.saturating_add(item_chars) > context_budget {
                    return Err(format!("conversation item {} exceeds the model context budget", items[end].item_id));
                }
                chunk_chars = chunk_chars.saturating_add(item_chars);
                end += 1;
            }
            if end == offset {
                return Err("memory update could not fit a conversation item in the model context".to_string());
            }
            let chunk = &items[offset..end];
            allowed_sources.extend(
                chunk
                    .iter()
                    .filter(|item| item.kind == ConversationItemKind::UserText)
                    .map(|item| item.item_id.clone()),
            );
            let source = chunk
                .iter()
                .filter_map(|item| {
                    render_memory_source_item(
                        item.kind,
                        &item.item_id,
                        item.content_text.as_deref().unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let source_chars = source.chars().count();
            let canonical = if full_chars.saturating_add(source_chars) <= context_budget {
                editable = all_editable;
                full
            } else {
                let query = chunk
                    .iter()
                    .filter_map(|item| item.content_text.as_deref())
                    .collect::<Vec<_>>()
                    .join(" ");
                for result in native_memory.search_relevant(&query, 8).map_err(|error| error.to_string())? {
                    let mut candidate = editable.clone();
                    include_page_and_ancestors(&canonical_pages, &result.path, &mut candidate);
                    let rendered = memory_prompt_catalog(&canonical_pages, &candidate)?;
                    if rendered.chars().count().saturating_add(source_chars) <= context_budget {
                        editable = candidate;
                    }
                }
                memory_prompt_catalog(&canonical_pages, &editable)?
            };
            let last = chunk.last().expect("non-empty chunk");
            let next_state = noema_memory::MemoryState {
                conversation_id: Some(conversation_id.to_string()),
                last_consolidated_sequence: last.sequence_index,
                last_consolidated_item: Some(last.item_id.clone()),
                updated_at: String::new(),
            };
            let mut correction = None;
            loop {
                let response = provider.generate_streaming(
                    GenerateRequest {
                        conversation_id: Some(conversation_id.to_string()),
                        model: selection.model_profile.clone(),
                        input: GenerateInput::Text(source.clone()),
                        instructions: Some(memory_update_instructions(&canonical, correction.as_deref())),
                        options: GenerateOptions { generation_priority: GenerationPriority::Background, max_output_tokens: Some(2_048), reasoning_effort: selection.reasoning_effort, ..GenerateOptions::default() },
                        tools: Vec::new(),
                        tool_transport: provider
                            .tool_capabilities(selection.model_profile.as_deref())
                            .tool_transport,
                        tool_choice: Default::default(), parallel_tool_calls: false,
                    },
                    &mut |_| {},
                ).await.map_err(|error| error.to_string())?;
                let changes = match parse_memory_change_set(
                    &response.assistant_text(),
                    &allowed_sources,
                    &canonical_pages,
                ) {
                    Ok(parsed) => {
                        let mut scoped_editable = editable.clone();
                        scoped_editable.extend(parsed.metadata_paths);
                        match validate_memory_change_scope(
                            &parsed.changes,
                            &canonical_pages,
                            &scoped_editable,
                        ) {
                            Ok(()) => parsed.changes,
                            Err(error) if correction.is_none() => {
                                correction = Some(error);
                                continue;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) if correction.is_none() => {
                        correction = Some(error);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                match native_memory.publish_with_state(&changes, &next_state) {
                    Ok(()) => break,
                    Err(error @ (noema_memory::NativeMemoryError::InvalidChangeSet(_)
                        | noema_memory::NativeMemoryError::InvalidPage(_))) if correction.is_none() => {
                            correction = Some(error.to_string());
                        }
                    Err(error) => return Err(error.to_string()),
                }
            }
            offset = end;
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct MemoryPromptPage<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    path: &'a str,
    title: &'a str,
    parent: Option<&'a str>,
    icon: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excerpt: Option<String>,
}

fn memory_prompt_catalog(
    pages: &[noema_memory::MemoryPage],
    editable: &HashSet<String>,
) -> Result<String, String> {
    let pages = pages
        .iter()
        .map(|page| {
            let selected = editable.contains(&page.path);
            MemoryPromptPage {
                id: selected.then_some(page.id.as_str()),
                path: &page.path,
                title: &page.title,
                parent: page.parent.as_deref(),
                icon: &page.icon,
                hash: selected.then_some(page.hash.as_str()),
                body: selected.then_some(page.body.as_str()),
                sources: selected.then_some(page.sources.as_slice()),
                excerpt: (!selected).then(|| bounded_memory_excerpt(&page.body)),
            }
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&pages).map_err(|error| error.to_string())
}

fn bounded_memory_excerpt(body: &str) -> String {
    let mut excerpt = body.chars().take(120).collect::<String>();
    if body.chars().count() > 120 {
        excerpt.push('…');
    }
    excerpt
}

fn include_page_and_ancestors(
    pages: &[noema_memory::MemoryPage],
    path: &str,
    selected: &mut HashSet<String>,
) {
    let mut current = Some(path);
    while let Some(path) = current {
        selected.insert(path.to_string());
        current = pages
            .iter()
            .find(|page| page.path == path)
            .and_then(|page| page.parent.as_deref());
    }
}

fn validate_memory_change_scope(
    changes: &noema_memory::MemoryChangeSet,
    pages: &[noema_memory::MemoryPage],
    editable: &HashSet<String>,
) -> Result<(), String> {
    for change in &changes.upserts {
        let current = change
            .id
            .as_deref()
            .and_then(|id| pages.iter().find(|page| page.id == id))
            .or_else(|| pages.iter().find(|page| page.path == change.path));
        if let Some(current) = current
            && !editable.contains(&current.path)
        {
            return Err(format!(
                "page {} was catalog-only and cannot be changed without its full body",
                current.path
            ));
        }
        if let Some(destination) = pages.iter().find(|page| page.path == change.path)
            && !editable.contains(&destination.path)
        {
            return Err(format!(
                "page {} was catalog-only and cannot be overwritten",
                destination.path
            ));
        }
    }
    for path in &changes.deletes {
        if pages.iter().any(|page| page.path == *path) && !editable.contains(path) {
            return Err(format!(
                "page {path} was catalog-only and cannot be deleted without its full body"
            ));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct ModelMemoryChangeSet {
    #[serde(default)]
    upserts: Vec<noema_memory::MemoryPageChange>,
    #[serde(default)]
    metadata_updates: Vec<MemoryMetadataUpdate>,
    #[serde(default)]
    deletes: Vec<String>,
}

#[derive(Deserialize)]
struct MemoryMetadataUpdate {
    path: String,
    icon: String,
}

#[derive(Debug)]
struct ParsedMemoryChangeSet {
    changes: noema_memory::MemoryChangeSet,
    metadata_paths: HashSet<String>,
}

fn render_memory_source_item(
    kind: ConversationItemKind,
    item_id: &str,
    content: &str,
) -> Option<String> {
    match kind {
        ConversationItemKind::UserText => Some(format!("human [{item_id}] {content}")),
        ConversationItemKind::AssistantText => Some(format!("assistant {content}")),
        _ => None,
    }
}

fn parse_memory_change_set(
    text: &str,
    allowed_sources: &std::collections::HashSet<String>,
    pages: &[noema_memory::MemoryPage],
) -> Result<ParsedMemoryChangeSet, String> {
    let json_start = text
        .find('{')
        .ok_or_else(|| "memory model returned no JSON change set".to_string())?;
    let json_end = text
        .rfind('}')
        .ok_or_else(|| "memory model returned incomplete JSON change set".to_string())?;
    let proposed: ModelMemoryChangeSet = serde_json::from_str(&text[json_start..=json_end])
        .map_err(|error| format!("invalid memory change set: {error}"))?;
    let mut changes = noema_memory::MemoryChangeSet {
        upserts: proposed.upserts,
        deletes: proposed.deletes,
    };
    for change in &mut changes.upserts {
        for source in &mut change.sources {
            if allowed_sources.contains(source.as_str()) {
                continue;
            }
            let qualified = format!("item:{source}");
            let Some(canonical) = allowed_sources.get(&qualified) else {
                continue;
            };
            change.body = change
                .body
                .split('\n')
                .map(|line| match line.split_once("]:") {
                    Some((label, target))
                        if label.starts_with("[^")
                            && target.trim().trim_matches('`') == source.as_str() =>
                    {
                        format!("{label}]: {canonical}")
                    }
                    _ => line.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            source.clone_from(canonical);
        }
    }
    if let Some(source) = changes
        .upserts
        .iter()
        .flat_map(|change| &change.sources)
        .find(|source| !allowed_sources.contains(*source))
    {
        return Err(format!(
            "memory change set cites source {source} that is neither existing provenance nor a human message in this chunk"
        ));
    }
    let mut metadata_paths = HashSet::new();
    for update in proposed.metadata_updates {
        let page = pages
            .iter()
            .find(|page| page.path == update.path)
            .ok_or_else(|| format!("metadata update references unknown page {}", update.path))?;
        if !noema_memory::MEMORY_PAGE_ICON_KEYS.contains(&update.icon.as_str()) {
            return Err(format!("unsupported memory icon {}", update.icon));
        }
        if !metadata_paths.insert(page.path.clone()) {
            return Err(format!("duplicate metadata update for {}", page.path));
        }
        if changes.deletes.contains(&page.path)
            || changes.upserts.iter().any(|change| {
                change.path == page.path || change.id.as_deref() == Some(page.id.as_str())
            })
        {
            return Err(format!(
                "page {} cannot have both a content and metadata operation",
                page.path
            ));
        }
        if page.icon == update.icon {
            continue;
        }
        changes.upserts.push(noema_memory::MemoryPageChange {
            id: Some(page.id.clone()),
            expected_hash: Some(page.hash.clone()),
            path: page.path.clone(),
            title: page.title.clone(),
            icon: update.icon,
            body: page.body.clone(),
            sources: page.sources.clone(),
        });
    }
    Ok(ParsedMemoryChangeSet {
        changes,
        metadata_paths,
    })
}

fn memory_update_instructions(canonical: &str, correction: Option<&str>) -> String {
    let correction = correction.map_or_else(String::new, |error| {
        format!("\nYour previous response was rejected: {error}. Correct that failure in the replacement response.")
    });
    let icon_keys = noema_memory::MEMORY_PAGE_ICON_KEYS.join(", ");
    format!(
        "You are editing a compact personal encyclopedia, not recording a chronological fact list. The complete page catalog is below. Entries with body and sources are content-editable and include stable ids and exact hashes; excerpt-only entries are discovery context and must not be content-upserted, moved, overwritten, or deleted, though their icon may be changed with metadata_updates. You may create a new page when the evidence warrants one. Existing pages are: {canonical}\n\
Return only JSON matching {{\"upserts\":[{{\"id\":null,\"expected_hash\":null,\"path\":\"relative.md\",\"title\":\"Human name or topic\",\"icon\":\"user\",\"body\":\"Two-to-four sentence lead that identifies the subject and combines its defining themes.[^identity]\\n\\n## Career and learning\\n\\nA cohesive paragraph relating several facts instead of isolating each claim.[^career]\\n\\n## Interests and daily life\\n\\nAnother cohesive paragraph.\\n\\n[^identity]: source-id-1\\n[^career]: source-id-2\",\"sources\":[\"source-id-1\",\"source-id-2\"]}}],\"metadata_updates\":[{{\"path\":\"existing.md\",\"icon\":\"briefcase-business\"}}],\"deletes\":[]}}.\n\
Editorial contract: root.md is a biographical overview titled with the local human's name whenever known, never \"Human memory\" in that case. Begin each page with a natural human-language lead, then group related material into thematic ## sections. A developed root article must have at least two sections. Merge related claims into multi-sentence prose; never emit a sequence of one-sentence fact paragraphs, a field inventory, or a chronology of messages. Keep the root concise and create focused child pages when a domain has enough detail, rather than accumulating every fact in root.md. Do not put a # title in body because Noema generates it. Rewrite any existing page that violates this structure even when its facts remain correct. Put all footnote definitions together after the article.\n\
Icon contract: every content upsert must include exactly one semantically specific Lucide icon key from [{icon_keys}]. Preserve an existing icon when it remains the clearest fit. When only an existing page's icon should change, emit one metadata_updates entry instead of reproducing its content; use this whenever another allowed key represents the stable page subject more clearly. Treat file-text as a generic fallback and replace it whenever a more specific key fits.\n\
Evidence contract: every cited footnote has one definition whose exact target is a source id, definitions exactly match sources, and assistant messages are context rather than independent evidence. Preserve stable ids, expected hashes, hierarchy, and user-authored meaning unless evidence requires a change. To move a page, retain its id and expected hash and change its path. Do not copy secrets, tokens, credentials, or private keys. Use owner human:local and scope human:local.{correction}"
    )
}

#[cfg(test)]
mod memory_change_set_tests {
    use super::*;

    fn prompt_page(path: &str, parent: Option<&str>, body: &str) -> noema_memory::MemoryPage {
        noema_memory::MemoryPage {
            id: format!("memory:human:{path}"),
            path: path.to_string(),
            title: path.to_string(),
            icon: "file-text".to_string(),
            body: body.to_string(),
            hash: format!("hash-{path}"),
            sources: vec![format!("source-{path}")],
            parent: parent.map(str::to_string),
            ancestors: Vec::new(),
            children: Vec::new(),
        }
    }

    #[test]
    fn memory_source_exposes_only_human_item_ids() {
        let human = render_memory_source_item(
            ConversationItemKind::UserText,
            "item:human",
            "Human evidence",
        )
        .expect("human source");
        let assistant = render_memory_source_item(
            ConversationItemKind::AssistantText,
            "item:assistant",
            "Assistant context",
        )
        .expect("assistant context");

        assert_eq!(human, "human [item:human] Human evidence");
        assert_eq!(assistant, "assistant Assistant context");
        assert!(!assistant.contains("item:assistant"));
    }

    #[test]
    fn parser_repairs_only_an_exact_missing_item_namespace() {
        let allowed = HashSet::from(["item:18c46bcd2ec74cc0f4".to_string()]);
        let response = r#"{"upserts":[{"path":"root.md","title":"Momo","icon":"user","body":"Momo corrected the agent's name.[^name]\n\n[^name]: 18c46bcd2ec74cc0f4","sources":["18c46bcd2ec74cc0f4"]}],"deletes":[]}"#;

        let changes = parse_memory_change_set(response, &allowed, &[])
            .expect("exact source alias")
            .changes;
        assert_eq!(
            changes.upserts[0].sources,
            ["item:18c46bcd2ec74cc0f4"]
        );
        assert!(
            changes.upserts[0]
                .body
                .contains("[^name]: item:18c46bcd2ec74cc0f4")
        );

        let unrelated = response.replace("18c46bcd2ec74cc0f4", "invented");
        assert!(parse_memory_change_set(&unrelated, &allowed, &[]).is_err());
    }

    #[test]
    fn parser_requires_memory_page_icons() {
        let response = r#"{"upserts":[{"path":"root.md","title":"Momo","body":"Momo has a memory.","sources":[]}],"deletes":[]}"#;

        let error = parse_memory_change_set(response, &HashSet::new(), &[])
            .expect_err("missing icon");
        assert!(error.contains("missing field `icon`"));
    }

    #[test]
    fn compact_catalog_keeps_every_page_and_expands_selected_ancestry() {
        let pages = vec![
            prompt_page("root.md", None, "Root biography"),
            prompt_page("career.md", None, "Career overview"),
            prompt_page("career/projects.md", Some("career.md"), "Project details"),
        ];
        let mut selected = HashSet::from(["root.md".to_string()]);
        include_page_and_ancestors(&pages, "career/projects.md", &mut selected);
        let catalog: serde_json::Value = serde_json::from_str(
            &memory_prompt_catalog(&pages, &selected).expect("catalog"),
        )
        .expect("catalog json");
        let entries = catalog.as_array().expect("entries");

        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|entry| entry.get("body").is_some()));
        assert!(entries.iter().all(|entry| entry.get("children").is_none()));

        let root_only = memory_prompt_catalog(&pages, &HashSet::from(["root.md".to_string()]))
            .expect("root catalog");
        assert!(root_only.contains("\"excerpt\":\"Project details\""));
        assert!(root_only.contains("\"icon\":\"file-text\""));
        assert!(!root_only.contains("\"body\":\"Project details\""));
    }

    #[test]
    fn constrained_updates_protect_catalog_only_pages_without_blocking_creates() {
        let pages = vec![
            prompt_page("root.md", None, "Root biography"),
            prompt_page("career.md", None, "Career overview"),
        ];
        let editable = HashSet::from(["root.md".to_string()]);
        let omitted_update = noema_memory::MemoryChangeSet {
            upserts: vec![noema_memory::MemoryPageChange {
                id: Some(pages[1].id.clone()),
                expected_hash: Some(pages[1].hash.clone()),
                path: pages[1].path.clone(),
                title: pages[1].title.clone(),
                icon: "briefcase-business".to_string(),
                body: pages[1].body.clone(),
                sources: pages[1].sources.clone(),
            }],
            deletes: Vec::new(),
        };
        assert!(validate_memory_change_scope(&omitted_update, &pages, &editable).is_err());

        let create = noema_memory::MemoryChangeSet {
            upserts: vec![noema_memory::MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "interests.md".to_string(),
                title: "Interests".to_string(),
                icon: "sparkles".to_string(),
                body: "A new evidence-backed topic.".to_string(),
                sources: Vec::new(),
            }],
            deletes: Vec::new(),
        };
        assert!(validate_memory_change_scope(&create, &pages, &editable).is_ok());
    }

    #[test]
    fn metadata_updates_merge_without_losing_page_content() {
        let pages = vec![
            prompt_page("root.md", None, "Root biography"),
            prompt_page("career.md", None, "Career overview"),
        ];
        let response = r#"model preface {"upserts":[],"metadata_updates":[{"path":"career.md","icon":"briefcase-business"}],"deletes":[]}"#;
        let parsed = parse_memory_change_set(response, &HashSet::new(), &pages)
            .expect("metadata changes");
        let changes = parsed.changes;

        assert_eq!(parsed.metadata_paths, HashSet::from(["career.md".to_string()]));
        assert_eq!(changes.upserts[0].body, pages[1].body);
        assert_eq!(changes.upserts[0].sources, pages[1].sources);
        assert_eq!(changes.upserts[0].expected_hash, Some(pages[1].hash.clone()));
        assert_eq!(changes.upserts[0].icon, "briefcase-business");
        let no_op = parse_memory_change_set(
            r#"{"metadata_updates":[{"path":"career.md","icon":"file-text"}]}"#,
            &HashSet::new(),
            &pages,
        )
        .expect("unchanged metadata");
        assert!(no_op.changes.upserts.is_empty());
        for invalid in [
            r#"{"metadata_updates":[{"path":"other.md","icon":"file-text"}]}"#,
            r#"{"metadata_updates":[{"path":"root.md","icon":"unknown"}]}"#,
            r#"{"metadata_updates":[{"path":"root.md","icon":"user"},{"path":"root.md","icon":"user"}]}"#,
            r#"{"upserts":[{"id":"memory:human:root.md","path":"root.md","title":"root.md","icon":"user","body":"Root biography","sources":[]}],"metadata_updates":[{"path":"root.md","icon":"user"}]}"#,
        ] {
            assert!(
                parse_memory_change_set(invalid, &HashSet::new(), &pages).is_err(),
                "{invalid}"
            );
        }
    }
}
