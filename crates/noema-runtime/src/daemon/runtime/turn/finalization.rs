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
                Ok(Some(_)) => {
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
                Ok(None) => {}
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
        let capabilities = provider.tool_capabilities(selection.model_profile.as_deref());
        let (memory_tools, memory_tool_choice) =
            super::typed_terminal_tools::required_native_tool(
                super::typed_terminal_tools::memory_changes_tool_spec()
                    .map_err(|error| format!("memory changes tool schema is invalid: {error}"))?,
                capabilities,
            )?;
        let context_budget = provider
            .context_metadata(selection.model_profile.as_deref())
            .await
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
                .flat_map(|page| page.citations.iter())
                .flat_map(|citation| citation.sources.iter().cloned())
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
                let item_chars = render_memory_source_item(&items[end])
                    .map_or(0, |source| source.chars().count())
                    .saturating_add(1);
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
                    .filter(|item| memory_evidence_payload(item).is_some())
                    .map(|item| item.item_id.clone()),
            );
            let source = chunk
                .iter()
                .filter_map(|item| {
                    render_memory_source_item(item)
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
                    .filter_map(|item| {
                        memory_evidence_payload(item).or_else(|| item.content_text.clone())
                    })
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
                        tools: memory_tools.clone(),
                        tool_transport: capabilities.tool_transport,
                        tool_choice: memory_tool_choice.clone(), parallel_tool_calls: false,
                    },
                    &mut |_| {},
                ).await.map_err(|error| error.to_string())?;
                let changes = match super::typed_terminal_tools::required_tool_payload::<
                    serde_json::Value,
                >(&response, super::typed_terminal_tools::SUBMIT_MEMORY_CHANGES_TOOL)
                .and_then(|payload| {
                    parse_memory_change_set(&payload, &allowed_sources, &canonical_pages)
                }) {
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
    citations: Option<&'a [noema_memory::MemoryCitation]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excerpt: Option<String>,
}

pub(crate) fn memory_prompt_catalog(
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
                citations: selected.then_some(page.citations.as_slice()),
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

pub(crate) fn validate_memory_change_scope(
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
#[serde(deny_unknown_fields)]
struct ModelMemoryChangeSet {
    #[serde(default)]
    upserts: Vec<ModelMemoryPageChange>,
    #[serde(default)]
    metadata_updates: Vec<MemoryMetadataUpdate>,
    #[serde(default)]
    deletes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelMemoryPageChange {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    expected_hash: Option<String>,
    path: String,
    title: String,
    icon: String,
    body: String,
    #[serde(default)]
    citations: Vec<noema_memory::MemoryCitation>,
}

impl From<ModelMemoryPageChange> for noema_memory::MemoryPageChange {
    fn from(change: ModelMemoryPageChange) -> Self {
        Self {
            id: change.id,
            expected_hash: change.expected_hash,
            path: change.path,
            title: change.title,
            icon: change.icon,
            body: change.body,
            citations: change.citations,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemoryMetadataUpdate {
    path: String,
    icon: String,
}

#[derive(Debug)]
pub(crate) struct ParsedMemoryChangeSet {
    pub(crate) changes: noema_memory::MemoryChangeSet,
    pub(crate) metadata_paths: HashSet<String>,
}

fn memory_evidence_payload(item: &noema_conversations::ConversationItemRecord) -> Option<String> {
    match item.kind {
        ConversationItemKind::UserText => item.content_text.clone(),
        ConversationItemKind::ToolResult => item
            .payload_json
            .pointer("/metadata/action/payload")
            .map(serde_json::Value::to_string),
        ConversationItemKind::Activity
            if item.payload_json.get("activity_kind").and_then(serde_json::Value::as_str)
                == Some("tool_result") =>
        {
            item.payload_json
                .pointer("/metadata/action/payload")
                .map(serde_json::Value::to_string)
        }
        _ => None,
    }
}

fn render_memory_source_item(
    item: &noema_conversations::ConversationItemRecord,
) -> Option<String> {
    match item.kind {
        ConversationItemKind::AssistantText => Some(format!(
            "assistant {}",
            item.content_text.as_deref().unwrap_or_default()
        )),
        ConversationItemKind::UserText => memory_evidence_payload(item)
            .map(|payload| format!("human [{}] {payload}", item.item_id)),
        ConversationItemKind::ToolResult | ConversationItemKind::Activity =>
            memory_evidence_payload(item)
                .map(|payload| format!("tool result [{}] {payload}", item.item_id)),
        _ => None,
    }
}

pub(crate) fn parse_memory_change_set(
    payload: &serde_json::Value,
    allowed_sources: &std::collections::HashSet<String>,
    pages: &[noema_memory::MemoryPage],
) -> Result<ParsedMemoryChangeSet, String> {
    let proposed: ModelMemoryChangeSet = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid memory change set: {error}"))?;
    let mut changes = noema_memory::MemoryChangeSet {
        upserts: proposed.upserts.into_iter().map(Into::into).collect(),
        deletes: proposed.deletes,
    };
    for change in &mut changes.upserts {
        normalize_memory_citations(change, allowed_sources)?;
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
            citations: page.citations.clone(),
        });
    }
    Ok(ParsedMemoryChangeSet {
        changes,
        metadata_paths,
    })
}

fn normalize_memory_citations(
    change: &mut noema_memory::MemoryPageChange,
    allowed_sources: &HashSet<String>,
) -> Result<(), String> {
    for citation in &mut change.citations {
        let mut seen_sources = HashSet::new();
        for source in &mut citation.sources {
            let canonical = canonical_memory_source(source, allowed_sources).ok_or_else(|| {
                format!(
                    "memory change set cites source {source} that is neither existing evidence nor an eligible item in this chunk"
                )
            })?;
            source.clone_from(canonical);
            if !seen_sources.insert(source.clone()) {
                return Err(format!(
                    "memory page {} repeats source {source} in one citation group",
                    change.path
                ));
            }
        }
    }

    let article = change
        .body
        .lines()
        .filter(|line| !is_numeric_footnote_definition(line))
        .collect::<Vec<_>>()
        .join("\n");
    let references = numeric_footnote_references(&article)?;
    let expected = (1..=change.citations.len()).collect::<std::collections::BTreeSet<_>>();
    if references != expected {
        return Err(format!(
            "memory page {} must use every citation group exactly by numeric marker; expected {expected:?}, found {references:?}",
            change.path
        ));
    }

    change.body = article.trim().to_string();
    Ok(())
}

fn is_numeric_footnote_definition(line: &str) -> bool {
    let Some((label, _)) = line.strip_prefix("[^").and_then(|line| line.split_once("]:")) else {
        return false;
    };
    label.parse::<usize>().is_ok_and(|index| index > 0)
}

fn numeric_footnote_references(body: &str) -> Result<std::collections::BTreeSet<usize>, String> {
    let mut references = std::collections::BTreeSet::new();
    let mut remainder = body;
    while let Some(start) = remainder.find("[^") {
        remainder = &remainder[start + 2..];
        let Some(end) = remainder.find(']') else {
            return Err("memory body contains an unterminated footnote marker".to_string());
        };
        let label = &remainder[..end];
        let index = label.parse::<usize>().map_err(|_| {
            format!("memory body footnote marker {label} is not a positive source index")
        })?;
        if index == 0 {
            return Err("memory body footnote indexes start at 1".to_string());
        }
        remainder = &remainder[end + 1..];
        if remainder.starts_with(':') {
            return Err("memory body must omit footnote definitions".to_string());
        }
        references.insert(index);
    }
    Ok(references)
}

fn canonical_memory_source<'a>(
    source: &str,
    allowed_sources: &'a std::collections::HashSet<String>,
) -> Option<&'a String> {
    allowed_sources.get(source).or_else(|| {
        let qualified = format!("item:{source}");
        allowed_sources.get(&qualified).or_else(|| {
            source
                .strip_prefix("human [")
                .and_then(|source| source.strip_suffix(']'))
                .or_else(|| {
                    source
                        .strip_prefix("tool result [")
                        .and_then(|source| source.strip_suffix(']'))
                })
                .and_then(|source| allowed_sources.get(source))
        })
    })
}

pub(crate) fn memory_update_instructions(canonical: &str, correction: Option<&str>) -> String {
    let correction = correction.map_or_else(String::new, |error| {
        format!("\nYour previous native tool call was rejected: {error}. Correct that failure in the replacement tool call.")
    });
    let icon_keys = noema_memory::MEMORY_PAGE_ICON_KEYS.join(", ");
    let article_word_target = noema_memory::MEMORY_MAX_WORDS.saturating_sub(100);
    format!(
        "You are editing a compact personal encyclopedia, not recording a chronological fact list. The complete page catalog is below. Entries with body and citations are content-editable and include stable ids and exact hashes; excerpt-only entries are discovery context and must not be content-upserted, moved, overwritten, or deleted, though their icon may be changed with metadata_updates. You may create a new page when the evidence warrants one. Existing pages are: {canonical}\n\
Call noema.submit_memory_changes exactly once through the provider's native tool channel. Do not encode the tool call or its arguments in ordinary assistant text.\n\
Editorial contract: root.md is a biographical overview titled with the local human's name whenever known, never \"Human memory\" in that case. Begin each page with a natural human-language lead, then group related material into thematic ## sections. A developed root article must have at least two sections. Merge related claims into multi-sentence prose; never emit a sequence of one-sentence fact paragraphs, a field inventory, or a chronology of messages. Keep the root concise and create focused child pages when a domain has enough detail, rather than accumulating every fact in root.md. Store stable human facts, preferences, relationships, and durable decisions. Do not store current connector readiness, enabled-tool counts, temporary failures, task execution history, project validation records, or researched subject facts that belong in their live object, task, project, document, or artifact. A rendered page includes its title and generated footnote definitions. It must contain at most {} Unicode words. Keep each article body at or below {article_word_target} words to leave space for generated content. Do not put a # title in body because Noema generates it. Rewrite any existing page that violates this structure even when its facts remain correct.\n\
Icon contract: every content upsert must include exactly one semantically specific Lucide icon key from [{icon_keys}]. Preserve an existing icon when it remains the clearest fit. When only an existing page's icon should change, emit one metadata_updates entry instead of reproducing its content; use this whenever another allowed key represents the stable page subject more clearly. Treat file-text as a generic fallback and replace it whenever a more specific key fits.\n\
Evidence contract: citations is the ordered list of evidence groups. Each group contains one or more exact source ids supporting one nearby claim. Cite the first group as [^1], the second as [^2], and so on. Use every group at least once. Reuse an exact source across groups only when it supports several claims. Keep the smallest direct evidence set. Do not retain an old source only because an earlier page used it. Do not write footnote definitions because Noema generates them. Human messages and exact tool results can be evidence. Assistant messages are context rather than independent evidence. Preserve stable ids, expected hashes, hierarchy, and user-authored meaning unless evidence requires a change. To move a page, retain its id and expected hash and change its path. Do not copy secrets, tokens, credentials, or private keys. Use owner human:local and scope human:local.{correction}",
        noema_memory::MEMORY_MAX_WORDS,
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
            citations: vec![noema_memory::MemoryCitation {
                sources: vec![format!("source-{path}")],
            }],
            parent: parent.map(str::to_string),
            ancestors: Vec::new(),
            children: Vec::new(),
        }
    }

    #[test]
    fn memory_source_exposes_human_and_tool_evidence_but_not_assistant_ids() {
        let item = |kind, id: &str, text: &str, payload| {
            noema_conversations::ConversationItemRecord {
                item_id: id.to_string(),
                conversation_id: "conversation:1".to_string(),
                turn_id: None,
                sequence_index: 1,
                cursor: "conversation_item:1".to_string(),
                kind,
                status: noema_conversations::ConversationItemStatus::Completed,
                content_text: Some(text.to_string()),
                payload_json: payload,
                metadata: serde_json::json!({}),
                created_at: String::new(),
            }
        };
        let human_item = item(ConversationItemKind::UserText, "item:human", "Human evidence", serde_json::json!({}));
        let assistant_item = item(ConversationItemKind::AssistantText, "item:assistant", "Assistant context", serde_json::json!({}));
        let tool_item = item(ConversationItemKind::ToolResult, "item:tool", "Tool result", serde_json::json!({"metadata": {"action": {"payload": {"value": 7}}}}));
        let human = render_memory_source_item(&human_item).expect("human source");
        let assistant = render_memory_source_item(&assistant_item).expect("assistant context");
        let tool = render_memory_source_item(&tool_item).expect("tool source");

        assert_eq!(human, "human [item:human] Human evidence");
        assert_eq!(assistant, "assistant Assistant context");
        assert!(!assistant.contains("item:assistant"));
        assert_eq!(tool, "tool result [item:tool] {\"value\":7}");
    }

    #[test]
    fn memory_instructions_reserve_space_below_the_page_word_limit() {
        let instructions = memory_update_instructions("[]", None);
        let article_word_target = noema_memory::MEMORY_MAX_WORDS.saturating_sub(100);

        assert!(instructions.contains(&format!(
            "at most {} Unicode words",
            noema_memory::MEMORY_MAX_WORDS
        )));
        assert!(instructions.contains(&format!("at or below {article_word_target} words")));
    }

    #[test]
    fn parser_groups_exact_canonical_sources_without_definitions() {
        let allowed = HashSet::from([
            "item:18c46bcd2ec74cc0f4".to_string(),
            "item:tool".to_string(),
        ]);
        let response = serde_json::json!({
            "upserts": [{
                "path": "root.md",
                "title": "Momo",
                "icon": "user",
                "body": "Momo corrected the agent's name.[^1]",
                "citations": [{"sources": ["18c46bcd2ec74cc0f4", "tool result [item:tool]"]}]
            }],
            "metadata_updates": [],
            "deletes": []
        });

        let changes = parse_memory_change_set(&response, &allowed, &[])
            .expect("exact source alias")
            .changes;
        assert_eq!(changes.upserts[0].citations[0].sources, ["item:18c46bcd2ec74cc0f4", "item:tool"]);
        assert_eq!(
            changes.upserts[0].body,
            "Momo corrected the agent's name.[^1]"
        );

        let unrelated = response.to_string().replace("18c46bcd2ec74cc0f4", "invented");
        let unrelated: serde_json::Value = serde_json::from_str(&unrelated).expect("json");
        assert!(parse_memory_change_set(&unrelated, &allowed, &[]).is_err());
    }

    #[test]
    fn parser_canonicalizes_exact_rendered_human_source_label() {
        let allowed = HashSet::from(["item:18c7c757f1f6fa3a5a7".to_string()]);
        let response = serde_json::json!({
            "upserts": [{
                "path": "root.md",
                "title": "Momo",
                "icon": "user",
                "body": "Momo has a durable preference.[^1]",
                "citations": [{"sources": ["human [item:18c7c757f1f6fa3a5a7]"]}]
            }],
            "metadata_updates": [],
            "deletes": []
        });

        let changes = parse_memory_change_set(&response, &allowed, &[])
            .expect("rendered human source label")
            .changes;

        assert_eq!(changes.upserts[0].citations[0].sources, ["item:18c7c757f1f6fa3a5a7"]);
        assert_eq!(changes.upserts[0].body, "Momo has a durable preference.[^1]");
    }

    #[test]
    fn parser_rejects_invalid_source_indexes_and_provenance() {
        let allowed = HashSet::from(["item:human".to_string()]);
        for (body, citations) in [
            ("Missing a marker.", serde_json::json!([{"sources": ["item:human"]}])),
            ("Named marker.[^name]", serde_json::json!([{"sources": ["item:human"]}])),
            ("Duplicate sources.[^1]", serde_json::json!([{"sources": ["item:human", "item:human"]}])),
            ("Unknown source.[^1]", serde_json::json!([{"sources": ["item:unknown"]}])),
        ] {
            let response = serde_json::json!({
                "upserts": [{
                    "path": "root.md",
                    "title": "Momo",
                    "icon": "user",
                    "body": body,
                    "citations": citations,
                }],
                "metadata_updates": [],
                "deletes": [],
            });
            assert!(parse_memory_change_set(&response, &allowed, &[]).is_err());
        }
    }

    #[test]
    fn parser_requires_memory_page_icons() {
        let response = serde_json::json!({
            "upserts": [{
                "path": "root.md",
                "title": "Momo",
                "body": "Momo has a memory.",
                "citations": []
            }],
            "metadata_updates": [],
            "deletes": []
        });

        let error = parse_memory_change_set(&response, &HashSet::new(), &[])
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
                citations: pages[1].citations.clone(),
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
                citations: Vec::new(),
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
        let response = serde_json::json!({
            "upserts": [],
            "metadata_updates": [{"path": "career.md", "icon": "briefcase-business"}],
            "deletes": []
        });
        let parsed = parse_memory_change_set(&response, &HashSet::new(), &pages)
            .expect("metadata changes");
        let changes = parsed.changes;

        assert_eq!(parsed.metadata_paths, HashSet::from(["career.md".to_string()]));
        assert_eq!(changes.upserts[0].body, pages[1].body);
        assert_eq!(changes.upserts[0].citations, pages[1].citations);
        assert_eq!(changes.upserts[0].expected_hash, Some(pages[1].hash.clone()));
        assert_eq!(changes.upserts[0].icon, "briefcase-business");
        let no_op = parse_memory_change_set(
            &serde_json::json!({
                "upserts": [],
                "metadata_updates": [{"path": "career.md", "icon": "file-text"}],
                "deletes": []
            }),
            &HashSet::new(),
            &pages,
        )
        .expect("unchanged metadata");
        assert!(no_op.changes.upserts.is_empty());
        for invalid in [
            serde_json::json!({"upserts": [], "metadata_updates": [{"path": "other.md", "icon": "file-text"}], "deletes": []}),
            serde_json::json!({"upserts": [], "metadata_updates": [{"path": "root.md", "icon": "unknown"}], "deletes": []}),
            serde_json::json!({"upserts": [], "metadata_updates": [{"path": "root.md", "icon": "user"}, {"path": "root.md", "icon": "user"}], "deletes": []}),
            serde_json::json!({"upserts": [{"id": "memory:human:root.md", "path": "root.md", "title": "root.md", "icon": "user", "body": "Root biography", "citations": []}], "metadata_updates": [{"path": "root.md", "icon": "user"}], "deletes": []}),
        ] {
            assert!(
                parse_memory_change_set(&invalid, &HashSet::new(), &pages).is_err(),
                "{invalid}"
            );
        }
    }
}
