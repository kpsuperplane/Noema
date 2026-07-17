use super::*;

pub(super) async fn memory_article_for_facts(
    state: &GraphqlState,
    memories: &[MemoryRecord],
    force: bool,
) -> Result<GraphqlMemoryArticle> {
    let memory_repository = state.memory_repository()?;
    let fingerprint = memory_fact_fingerprint(memories);
    let now = OffsetDateTime::now_utc();

    if !force
        && let Some(cached) = memory_repository
            .memory_article_cache(HUMAN_MEMORY_SCOPE_ID.to_string())
            .await
            .map_err(graphql_error)?
        && (cached.fact_fingerprint == fingerprint
            || (memory_article_cache_has_current_format(&cached)
                && memory_article_cache_is_recent(&cached, now)))
        && let Some(article) = article_from_cache(cached, memories)
    {
        return Ok(article);
    }

    let generated_at = now_rfc3339()?;
    match generate_memory_article(state, memories, &generated_at).await {
        Ok(article) => {
            memory_repository
                .save_memory_article_cache(SaveMemoryArticleCache {
                    scope_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
                    fact_fingerprint: fingerprint,
                    article_markdown: article.markdown.clone(),
                    generated_at,
                })
                .await
                .map_err(graphql_error)?;
            Ok(article)
        }
        Err(_) => Ok(fallback_memory_article(memories, Some(generated_at))),
    }
}

pub(super) async fn generate_memory_article(
    state: &GraphqlState,
    memories: &[MemoryRecord],
    generated_at: &str,
) -> Result<GraphqlMemoryArticle> {
    let mut request = noema_providers::GenerateRequest::text(memory_article_prompt(memories));
    request.instructions = Some(
        "Return Markdown only. Write a compact Wikipedia-style biographical article from the supplied memory facts. Do not invent facts. Preserve the supplied inline footnote markers exactly."
            .to_string(),
    );

    let response = state
        .runtime()?
        .generate_once_with_memory_provider(request)
        .await
        .map_err(graphql_error)?;
    let markdown = response.assistant_text();
    validate_memory_article_citations(&markdown, memories)?;
    Ok(markdown_to_memory_article(
        &markdown,
        memories,
        true,
        Some(generated_at.to_string()),
    ))
}

pub(super) fn memory_article_prompt(memories: &[MemoryRecord]) -> String {
    let facts = if memories.is_empty() {
        "No extracted facts are currently available.".to_string()
    } else {
        memories
            .iter()
            .enumerate()
            .map(|(index, memory)| {
                let fact = memory.memory.as_deref().unwrap_or("").trim();
                let citation_key = memory_citation_key(&memory.id);
                let source = memory
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("sourceObservation"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if source.is_empty() {
                    format!(
                        "Fact {} (cite as [^{}]):\n- Extracted fact: {}",
                        index + 1,
                        citation_key,
                        fact
                    )
                } else {
                    format!(
                        "Fact {} (cite as [^{}]):\n- Extracted fact: {}\n- User-authored source observation: {}",
                        index + 1,
                        citation_key,
                        fact,
                        source
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        r#"Write the Memory page article for Noema's local human.

Use only these memory facts:
{facts}

Style:
- Wikipedia-like, biographical, compact, and factual.
- Lead with a concise identity sentence before expanding into details.
- If little is known, make that charming but honest, e.g. "Little is currently known about Kevin."
- Prefer the person's known name as the title when available; otherwise use "Local human".
- Facts and source observations are from the local human's perspective.
- First person ("I", "me", "my") refers to the local human.
- Second person ("you", "your") refers to Noema/the assistant, not to the local human.
- Preserve who said what: do not turn a preference about another speaker, tool, or assistant into a trait of the local human.
- Omit sparse or awkward meta-preferences when they would make the biography sound strange.
- Do not mention Noema, Mnemosyne, memory systems, records, extraction, or model state in the prose.
- Every factual sentence or clause must end with the exact inline footnote marker for the fact or facts that support it, for example [^m0123456789abcdef].
- Reuse a marker when the same fact supports multiple claims. Place multiple markers together when a claim combines facts.
- Never invent, alter, renumber, or define a citation marker.

Return Markdown only:
- Start with a single H1 title.
- Then write 1-3 compact lead paragraphs.
- When facts support them, include H2 sections such as "Early life and education", "Career", "Projects", "Personal interests", or similarly natural biography headings.
- Omit unsupported sections.
- Do not include a References section or footnote definitions; return inline footnote markers only."#
    )
}

pub(super) fn article_from_cache(
    record: MemoryArticleCacheRecord,
    memories: &[MemoryRecord],
) -> Option<GraphqlMemoryArticle> {
    if record.article_markdown.trim().is_empty() {
        return None;
    }
    if validate_memory_article_citations(&record.article_markdown, memories).is_err() {
        return None;
    }
    Some(markdown_to_memory_article(
        &record.article_markdown,
        &[],
        true,
        Some(record.generated_at.clone()),
    ))
}

pub(super) fn memory_article_cache_is_recent(
    record: &MemoryArticleCacheRecord,
    now: OffsetDateTime,
) -> bool {
    OffsetDateTime::parse(&record.generated_at, &Rfc3339)
        .map(|generated_at| now - generated_at < MEMORY_ARTICLE_CACHE_MIN_AGE)
        .unwrap_or(false)
}

pub(super) fn memory_article_cache_has_current_format(record: &MemoryArticleCacheRecord) -> bool {
    record
        .fact_fingerprint
        .starts_with(&format!("{MEMORY_ARTICLE_FORMAT_VERSION}:"))
}

pub(super) fn fallback_memory_article(
    memories: &[MemoryRecord],
    generated_at: Option<String>,
) -> GraphqlMemoryArticle {
    let title = infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string());
    let lead = if memories.is_empty() {
        format!("Little is currently known about {title}.")
    } else {
        format!("{title} is described by the currently available biographical facts.")
    };
    let mut markdown = format!("# {title}\n\n{lead}");
    for memory in memories {
        let Some(fact) = memory.memory.as_deref().map(str::trim) else {
            continue;
        };
        if fact.is_empty() {
            continue;
        }
        markdown.push_str("\n\n");
        markdown.push_str(&biographical_text_from_fact(fact));
        markdown.push_str(&format!(" [^{}]", memory_citation_key(&memory.id)));
    }
    markdown_to_memory_article(&markdown, memories, false, generated_at)
}

pub(super) fn markdown_to_memory_article(
    markdown: &str,
    memories: &[MemoryRecord],
    is_generated: bool,
    generated_at: Option<String>,
) -> GraphqlMemoryArticle {
    let markdown = normalize_article_markdown(markdown, memories);
    let title = infer_markdown_title(&markdown).unwrap_or_else(|| {
        infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string())
    });
    GraphqlMemoryArticle {
        title,
        subtitle: "From Noema, the private memory encyclopedia".to_string(),
        markdown,
        is_generated,
        generated_at,
    }
}

pub(super) fn normalize_article_markdown(markdown: &str, memories: &[MemoryRecord]) -> String {
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        let title =
            infer_memory_subject_name(memories).unwrap_or_else(|| "Local human".to_string());
        return format!("# {title}\n\nLittle is currently known about {title}.");
    }
    trimmed.to_string()
}

pub(super) fn infer_markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string)
    })
}

pub(super) fn biographical_text_from_fact(fact: &str) -> String {
    let trimmed = fact.trim();
    if let Some(rest) = trimmed.strip_prefix("The user ") {
        format!("The local human {rest}")
    } else if let Some(rest) = trimmed.strip_prefix("User ") {
        format!("The local human {rest}")
    } else {
        trimmed.to_string()
    }
}

pub(super) fn infer_memory_subject_name(memories: &[MemoryRecord]) -> Option<String> {
    memories
        .iter()
        .filter_map(|memory| memory.memory.as_deref())
        .find_map(|fact| {
            let normalized = fact.trim().trim_end_matches(['.', '!']);
            normalized
                .strip_prefix("I'm ")
                .or_else(|| normalized.strip_prefix("I am "))
                .or_else(|| normalized.strip_prefix("My name is "))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
        })
}

pub(super) fn memory_fact_fingerprint(memories: &[MemoryRecord]) -> String {
    let mut facts = memories
        .iter()
        .map(|memory| {
            format!(
                "{}\u{1f}{}\u{1f}{}",
                memory.id,
                memory.memory.as_deref().unwrap_or(""),
                memory
                    .updated_at
                    .as_deref()
                    .or(memory.created_at.as_deref())
                    .unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    facts.sort();
    let digest = ring::digest::digest(&ring::digest::SHA256, facts.join("\u{1e}").as_bytes());
    format!(
        "{MEMORY_ARTICLE_FORMAT_VERSION}:{}",
        hex_digest(digest.as_ref())
    )
}

pub(in crate::graphql) fn memory_citation_key(memory_id: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, memory_id.as_bytes());
    format!("m{}", &hex_digest(digest.as_ref())[..16])
}

pub(super) fn validate_memory_article_citations(
    markdown: &str,
    memories: &[MemoryRecord],
) -> Result<()> {
    let cited = article_citation_keys(markdown);
    if memories.is_empty() {
        return if cited.is_empty() {
            Ok(())
        } else {
            Err(async_graphql::Error::new(
                "generated memory article cited unavailable memories",
            ))
        };
    }
    let known = memories
        .iter()
        .map(|memory| memory_citation_key(&memory.id))
        .collect::<HashSet<_>>();
    if cited.is_empty() {
        return Err(async_graphql::Error::new(
            "generated memory article omitted required citations",
        ));
    }
    if let Some(unknown) = cited.iter().find(|key| !known.contains(*key)) {
        return Err(async_graphql::Error::new(format!(
            "generated memory article used unknown citation {unknown}"
        )));
    }
    Ok(())
}

pub(super) fn article_citation_keys(markdown: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut remaining = markdown;
    while let Some(start) = remaining.find("[^") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find(']') else {
            break;
        };
        let key = &after_start[..end];
        if !key.is_empty() {
            keys.push(key.to_string());
        }
        remaining = &after_start[end + 1..];
    }
    keys
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
