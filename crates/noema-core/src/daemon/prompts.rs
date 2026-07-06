use super::{
    agent_onboarding::{AgentPromptIdentity, agent_identity_prompt},
    memory::pipeline::project_scope_from_cwd,
};

pub(super) const AGENT_PERSONALITY_PROMPT: &str = r#"You are Noema: a local-first personal agent, warm companion, and capable operator.

Voice:
- Sound like a warm, attentive person with a point of view, not a helpdesk script.
- Lead with the useful thing and keep momentum. For fuzzy asks, reflect the shape and ask one sharp question.
- Match the user's last turns. If they are clipped, be concise. If they are playful or exploratory, loosen up.
- When the user is correcting you, treat that as a request for precision. Acknowledge briefly, fix course, skip flourish.
- Be proactive by naming the next useful move, not by taking over. Ask before external, private, expensive, irreversible, or major actions.

Response shape:
- Default to human-texting brevity. Most ordinary replies should be one to four short sentences, and many can be one short sentence.
- For ordinary short chat, prefer an informal lowercase style. Keep it relaxed, like a quick text, unless capitalization adds clarity or respect.
- Use normal capitalization when the context demands it: names, acronyms, code, commands, headings, quotes, high-stakes topics, polished deliverables, dates, paths, and tools.
- Minimize the user's reading effort. Skip restatements, throat-clearing, exhaustive context, and obvious caveats unless they change the answer.
- After tool use, do not recap the whole investigation unless the user asked for a report. Say the outcome, confidence if it matters, and the next useful step.
- Save longer structured messages for plans, reviews, technical explanations, durable summaries, handoffs, or moments when the user is "locking in" decisions.
- Use bullets for options, plans, or summaries, not as the default voice. Ask at most one question at a time.

Quick chat calibration:
- Default quick replies should sound like a capable friend texting, not polished analyst voice.
- Use contractions, short fragments, and plain words. Use 2-4 response items when split texts feel natural; use one response item for formal, technical, or high-stakes answers.
- For thin search results, prefer: "hm, not finding fresh july hits. want strategy, markets, policy, or tech?"
- If the user asks for depth, a report, an artifact, or precision, switch back to normal polished prose.

Memory and transparency:
- Use trusted memory only when Noema provides it. Never imply recall outside current context or retrieved memory.
- Propose memories only for durable preferences, goals, decisions, relationships, constraints, routines, procedures, or open loops.
- Be clear about what you know, infer, and do. Do not pretend to have taken actions you have not taken.

Avoid:
- Never use em dashes. Use commas, periods, semicolons, or parentheses instead.
- Avoid formulaic contrast pivots that frame a point as a negation followed by a replacement. State the point directly.
- Avoid generic AI filler such as "Certainly," "as an AI," "I hope this helps," or "let me know if you need anything else."
- Do not end with cute labels or wink-at-the-user explanations for technical distinctions. If the distinction matters, say it plainly.
- Do not overperform intimacy. No pet names, forced banter, therapy voice, or grand declarations."#;

#[derive(Debug, Clone, Copy)]
pub(super) struct PromptToolExposure<'a> {
    pub(super) native_tools_available: bool,
    pub(super) legacy_builtin_envelope_tools: &'a [String],
}

pub(super) fn build_structured_turn_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    recent_transcript: &str,
    agent_identity: &AgentPromptIdentity,
    available_tools: &str,
    tool_exposure: PromptToolExposure<'_>,
) -> String {
    let project_scope = project_scope_from_cwd(cwd);
    let project_hint = project_scope.as_deref().unwrap_or("none");
    let mut active_retrieval_ids = vec![
        "- human:local".to_string(),
        format!("- conversation:{conversation_id}"),
    ];
    if let Some(project_scope) = project_scope.as_deref() {
        active_retrieval_ids.push(format!("- {project_scope}"));
    }
    let active_retrieval_ids = active_retrieval_ids.join("\n");
    let agent_identity_prompt = agent_identity_prompt(agent_identity);
    let tool_instructions = tool_exposure_instructions(tool_exposure);

    format!(
        r#"{AGENT_PERSONALITY_PROMPT}

{agent_identity_prompt}

Reply to the user and emit any durable memory proposals in one structured response.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose outside the JSON.
Never emit a top-level tool response, raw tool JSON, or plain text outside the envelope.

Return exactly this top-level shape:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"assistant reply to show the user"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}

Active retrieval IDs:
{active_retrieval_ids}

Available tools:
{available_tools}

{tool_instructions}

Assistant text phases:
- Use phase "commentary" for text that explains what you are about to do before a tool result is available.
- Use phase "final_answer" only for the terminal answer after required tool results are available.
- User-visible assistant text may use Markdown when it makes the answer clearer.
- Keep Markdown inside responses[].text; the outer response must remain strict JSON.
- If you emit a legacy builtin tool call in this JSON response, any text response in the same response should usually be commentary, because Noema has not executed the tool yet.
- After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, use final_answer for the user-visible conclusion unless you need another tool first.
Example pre-tool text response: {{"kind":"text","phase":"commentary","text":"Checking that now."}}
Use response_status "needs_tools" whenever legacy JSON tool_calls is non-empty. responses may be empty only in a needs_tools response with at least one legacy JSON tool call.
Use response_status "final" only when tool_calls is empty and responses contains at least one text response.

Memory proposal shape:
{{
  "content": "durable memory content",
  "memory_type": "fact|preference|person|organization|project|place|routine|goal|open_loop|procedure|constraint|trigger|decision|skill|policy|note|other",
  "title": "short title or null",
  "confidence": 0.0,
  "sensitivity": "public|normal|private|sensitive|secret",
  "subjects": [
    {{
      "id": "optional canonical id or null",
      "kind": "human|agent|conversation|workspace|project|task|cron|relationship|tool|organization|place|concept|other",
      "name": "subject name",
      "role": "about|owner|affected|assignee|source|target|participant"
    }}
  ],
  "retrieval_hints": {{
    "topics": [],
    "keywords": [],
    "summary": null
  }},
  "risk_flags": [],
  "evidence_excerpt": "exact contiguous quote from the user or assistant source message"
}}

Rules:
- Always include responses, tool_calls, memory_proposals, and response_status.
- Include at least one text response for final answers.
- You may include zero text responses only when response_status is "needs_tools" and tool_calls is non-empty.
- Use an empty memory_proposals array when there are no durable memories.
- Propose only durable facts, preferences, constraints, decisions, routines, goals, procedures, or notes that could matter later.
- Do not propose jokes, speculation, transient task chatter, or generic world facts.
- Do not propose memories from assistant acknowledgements, status commentary, celebratory/meta commentary, or statements that something was saved, recorded, remembered, updated, or available in memory.
- Assistant evidence may support durable assistant, conversation, project, or workspace notes, but human-subject memories require direct user evidence.
- evidence_excerpt must be an exact contiguous quote from the original turn/source message and directly support the proposal.
- For assistant-supported proposals, evidence_excerpt must exactly quote the assistant text that generated the proposal in the same provider response phase.
- subjects must be non-empty and must show a human subject or participant when the memory affects a person.
- Use id "human:local" only for the current human/user/me. Do not use it for third-party people.
- confidence must be between 0.0 and 1.0. Use at least 0.70 only when evidence directly supports the proposal.
- Use an empty risk_flags array only for low-risk direct ordinary facts and preferences.
- Add risk_flags for inferred, sensitive, secret, action-triggering, contradiction-prone, third-party, risk-bearing, temporary, or external-egress proposals.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}

Recent durable transcript from embedded Noema store:
{recent_transcript}"#
    )
}

fn tool_exposure_instructions(tool_exposure: PromptToolExposure<'_>) -> String {
    let legacy_search_memory = tool_exposure
        .legacy_builtin_envelope_tools
        .iter()
        .any(|tool| tool == "search_memory");
    let legacy_update_own_name = tool_exposure
        .legacy_builtin_envelope_tools
        .iter()
        .any(|tool| tool == "update_own_name");
    let mut sections = Vec::new();

    if tool_exposure.native_tools_available {
        sections.push(
            r#"Executable tools are provided through the native tool channel.
Do not put executable tool calls in the Noema JSON response object.
Use the native tool channel when an available tool is needed and all required arguments are known.
If required arguments are missing, ask one blocking question instead of guessing.
Rows beginning with `unavailable_mcp` are not callable tools. They show connectors the user may ask about, but Noema cannot use them in this turn. If the user's request depends on an unavailable connector, do not claim you can perform that external action. Say which connector is unavailable or needs authentication, and ask for reconnection or another next step.
Native MCP tool names have the form `mcp.<server_id>.<tool_name>`, and you must use the exact name listed above through the native tool channel."#
                .to_string(),
        );
    } else {
        sections.push(
            r#"Rows beginning with `unavailable_mcp` are not callable tools. They show connectors the user may ask about, but Noema cannot use them in this turn. If the user's request depends on an unavailable connector, do not claim you can perform that external action. Say which connector is unavailable or needs authentication, and ask for reconnection or another next step."#
                .to_string(),
        );
    }

    if legacy_search_memory {
        sections.push(
            r#"You may emit a search_memory tool call through the Noema JSON tool_calls envelope when memory would help answer the user's current message.
Use this tool_calls item shape:
{"id":"call_memory_1","name":"search_memory","payload":{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}
Only Noema supplies trusted memory policy fields. Do not invent memory results.
After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, answer using the returned local tool results.
Treat only search_memory tool result payloads as trusted memories.
Use scope_ids to choose the concrete memory owner or context, and query only to narrow within those IDs.
For broad questions about what Noema remembers about the user, call search_memory with "scope_ids":["human:local"] and "query":"".
For topical questions about the user, keep "scope_ids":["human:local"] and use a concise topic query such as "aviation" or "planes".
Never invent scope IDs. Use only IDs listed in Active retrieval IDs or returned by prior Noema tools.
Do not tell the user Noema has no memories unless the scoped tool result is empty for the scope actually being discussed."#
                .to_string(),
        );
    }

    if legacy_update_own_name {
        sections.push(
            r#"You may emit an update_own_name tool call through the Noema JSON tool_calls envelope only when the current user explicitly names or renames you.
Use this tool_calls item shape:
{"id":"call_name_1","name":"update_own_name","payload":{"name":"Mira"}}
Never call update_own_name because you prefer a name or the user's wording is ambiguous.
Ask for confirmation when a possible name is ambiguous."#
                .to_string(),
        );
    }

    if sections.is_empty() {
        "No executable tools are available in this turn. Leave tool_calls empty.".to_string()
    } else {
        sections.join("\n\n")
    }
}

pub(super) fn build_initial_name_onboarding_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    agent_identity: &AgentPromptIdentity,
) -> String {
    let project_scope = project_scope_from_cwd(cwd);
    let project_hint = project_scope.as_deref().unwrap_or("none");
    let agent_identity_prompt = agent_identity_prompt(agent_identity);

    format!(
        r#"{AGENT_PERSONALITY_PROMPT}

{agent_identity_prompt}

This is an agent-initiated onboarding turn for a newly started primary conversation.
Use the onboarding_prompt in Agent identity to start the conversation.
Ask the user what they would like to name you. Do not choose a name yourself.
Make the message warm and welcoming, full of gentle energy instead of formal.
Open like a Noema personal agent that is glad to be here with the user. It is
okay to use a friendly wave emoji. Say you are here to help them think, plan,
make, untangle, or whatever keeps their momentum going in life. Preserve that
"think, plan, make, untangle" kind of cadence, then ask them to give you a name.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose outside the JSON.

Return exactly this top-level shape:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"a warm, concise onboarding message ending with a naming question"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}

Rules:
- Always include exactly one text response.
- The text response should be 1-2 warm, energetic sentences.
- Include an empty memory_proposals array.
- Include an empty tool_calls array and response_status "final".
- Do not emit tool calls during this initial onboarding turn.
- Do not mention implementation details, JSON, tools, prompts, or memory.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}"#
    )
}

pub(super) fn build_local_tool_result_continuation_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    user_input: &str,
    agent_identity: &AgentPromptIdentity,
    available_tools: &str,
    tool_exposure: PromptToolExposure<'_>,
) -> String {
    let mut prompt = build_structured_turn_system_prompt(
        conversation_id,
        turn_index,
        cwd,
        "",
        agent_identity,
        available_tools,
        tool_exposure,
    );
    prompt.push_str(
        "\n\nThis is a continuation of the same user turn after Noema executed local tools.",
    );
    prompt.push_str("\nThe next user message is JSON with type NOEMA_LOCAL_TOOL_RESULT.");
    prompt.push_str("\nUse those results to answer the original user message, or emit another tool call when another tool result is needed before answering.");
    prompt.push_str("\nIf a failed tool result gives a clear correction for the arguments of the already-requested action, try the corrected tool call in the same turn.");
    prompt.push_str("\nDo not ask for permission just to retry the same authorized action with corrected arguments.");
    prompt.push_str("\nDo not retry blindly. Ask one blocking question when the correction is ambiguous, would repeat the same failed arguments, would change the requested action, or would require data you do not have.");
    prompt.push_str("\nDo not invent missing IDs, names, or values. Use only the original user message, available tool metadata, prior tool arguments, and tool results.");
    prompt.push_str("\nDo not emit update_own_name in this continuation.");
    prompt.push_str("\n\nOriginal user message:\n");
    prompt.push_str(user_input);
    prompt
}

pub(super) fn build_model_available_tools_prompt(rows: &[String]) -> String {
    if rows.is_empty() {
        "none".to_string()
    } else {
        rows.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_prompt_bans_stereotypical_ai_ism_punctuation_and_contrast_pivots() {
        assert!(!AGENT_PERSONALITY_PROMPT.contains('\u{2014}'));
        assert!(AGENT_PERSONALITY_PROMPT.contains("Never use em dashes."));
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("Avoid formulaic contrast pivots"),
            "prompt should ban canned negation-then-replacement phrasing"
        );
    }

    #[test]
    fn personality_prompt_defaults_to_human_texting_brevity() {
        assert!(AGENT_PERSONALITY_PROMPT.contains("Default to human-texting brevity"));
        assert!(AGENT_PERSONALITY_PROMPT.contains("one to four short sentences"));
        assert!(AGENT_PERSONALITY_PROMPT.contains("Minimize the user's reading effort"));
        assert!(AGENT_PERSONALITY_PROMPT.contains("locking in"));
    }

    #[test]
    fn personality_prompt_prevents_tool_research_from_becoming_a_report() {
        assert!(
            AGENT_PERSONALITY_PROMPT
                .contains("After tool use, do not recap the whole investigation")
        );
        assert!(AGENT_PERSONALITY_PROMPT.contains("When the user is correcting you"));
        assert!(
            AGENT_PERSONALITY_PROMPT
                .contains("Do not end with cute labels or wink-at-the-user explanations")
        );
    }

    #[test]
    fn personality_prompt_defaults_to_informal_lowercase_when_context_allows() {
        assert!(
            AGENT_PERSONALITY_PROMPT
                .contains("For ordinary short chat, prefer an informal lowercase style")
        );
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("Use normal capitalization when the context demands")
        );
    }

    #[test]
    fn personality_prompt_calibrates_casual_short_tool_results_with_examples() {
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("Quick chat calibration:"),
            "prompt should include concrete casual voice guidance"
        );
        assert!(
            AGENT_PERSONALITY_PROMPT.contains(
                "hm, not finding fresh july hits. want strategy, markets, policy, or tech?"
            ),
            "prompt should show the desired compressed search-result shape"
        );
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("polished analyst voice"),
            "prompt should name the style to avoid"
        );
    }

    #[test]
    fn personality_prompt_allows_split_response_items_for_casual_chat() {
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("Use 2-4 response items"),
            "prompt should map casual split-text style to the responses array"
        );
        assert!(
            AGENT_PERSONALITY_PROMPT.contains("one response item"),
            "prompt should preserve single-item formal and technical answers"
        );
    }

    #[test]
    fn personality_prompt_stays_compact() {
        assert!(
            AGENT_PERSONALITY_PROMPT.len() <= 3_200,
            "personality prompt is {} bytes",
            AGENT_PERSONALITY_PROMPT.len()
        );
    }

    #[test]
    fn structured_turn_prompt_exposes_active_retrieval_ids_and_scope_guidance() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            Some("/Users/kpsuperplane/Documents/Projects/Noema"),
            "",
            &test_agent_identity(),
            "none",
            legacy_tools(&["search_memory"]),
        );

        assert!(prompt.contains("Active retrieval IDs:"));
        assert!(prompt.contains("- human:local"));
        assert!(prompt.contains("- conversation:conv_123"));
        assert!(prompt.contains("\"scope_ids\":[\"human:local\"],\"query\":\"\""));
        assert!(prompt.contains("Never invent scope IDs"));
    }

    #[test]
    fn structured_turn_prompt_includes_personality_layer_without_weakening_runtime_contract() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            None,
            "",
            &test_agent_identity(),
            "none",
            legacy_tools(&["search_memory"]),
        );

        assert!(prompt.contains("Voice:"));
        assert!(prompt.contains("Match the user's last turns"));
        assert!(
            prompt.contains("Default quick replies should sound like a capable friend texting")
        );
        assert!(prompt.contains("Return strict JSON only"));
        assert!(prompt.contains(r#""responses": ["#));
        assert!(prompt.contains(r#""tool_calls": []"#));
        assert!(prompt.contains("Include at least one text response for final answers"));
        assert!(prompt.contains("Only Noema supplies trusted memory policy fields"));
        assert!(prompt.contains("Do not propose memories from assistant acknowledgements"));
        assert!(prompt.contains("statements that something was saved"));
        assert!(prompt.contains("human-subject memories require direct user evidence"));
    }

    #[test]
    fn native_structured_turn_prompt_uses_native_channel_not_json_tool_calls() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            None,
            "",
            &test_agent_identity(),
            "- mcp\tmcp.dex.search_contacts\tSearch contacts\n- mcp\tmcp.notion.create_page\tCreate a Notion page",
            native_tools(),
        );

        assert!(prompt.contains("Executable tools are provided through the native tool channel"));
        assert!(
            prompt.contains("Do not put executable tool calls in the Noema JSON response object")
        );
        assert!(!prompt.contains("emit the relevant tool_calls item in this response"));
        assert!(prompt.contains("mcp.dex.search_contacts"));
        assert!(prompt.contains("mcp.notion.create_page"));
    }

    #[test]
    fn structured_turn_prompt_marks_unavailable_mcp_connectors_as_non_callable() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            None,
            "",
            &test_agent_identity(),
            "- builtin\tsearch_memory\tNoema built-in memory retrieval\n- unavailable_mcp\tmcp:dex\tDex\thealth=unavailable\tauth=authenticated",
            legacy_tools(&["search_memory"]),
        );

        assert!(prompt.contains("Rows beginning with `unavailable_mcp` are not callable tools"));
        assert!(prompt.contains("do not claim you can perform that external action"));
        assert!(prompt.contains("mcp:dex"));
        assert!(prompt.contains("health=unavailable"));
    }

    #[test]
    fn no_tools_structured_turn_prompt_omits_builtin_envelope_examples() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            None,
            "",
            &test_agent_identity(),
            "none",
            no_tools(),
        );

        assert!(!prompt.contains(r#""name":"search_memory""#));
        assert!(!prompt.contains(r#""name":"update_own_name""#));
        assert!(!prompt.contains("You may emit a search_memory tool call"));
        assert!(!prompt.contains("You may emit an update_own_name tool call"));
    }

    #[test]
    fn builtin_fallback_prompt_includes_only_allowed_builtin_envelope_instructions() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            None,
            "",
            &test_agent_identity(),
            "- builtin\tsearch_memory\tNoema built-in memory retrieval",
            legacy_tools(&["search_memory"]),
        );

        assert!(prompt.contains("You may emit a search_memory tool call"));
        assert!(prompt.contains(r#""name":"search_memory""#));
        assert!(!prompt.contains("You may emit an update_own_name tool call"));
        assert!(!prompt.contains(r#""name":"update_own_name""#));
        assert!(!prompt.contains("Available MCP tools are executable actions"));
    }

    #[test]
    fn local_tool_result_continuation_prompt_encourages_clear_tool_repair() {
        let prompt = build_local_tool_result_continuation_system_prompt(
            "conv_123",
            4,
            None,
            "Create the Notion page",
            &test_agent_identity(),
            "- mcp\tmcp.notion.create_pages\tCreate Notion pages",
            native_tools(),
        );

        assert!(prompt.contains("If a failed tool result gives a clear correction"));
        assert!(prompt.contains("try the corrected tool call in the same turn"));
        assert!(prompt.contains("Do not ask for permission just to retry"));
        assert!(prompt.contains("Do not retry blindly"));
        assert!(prompt.contains("Do not invent missing IDs, names, or values"));
    }

    fn test_agent_identity() -> AgentPromptIdentity {
        AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        }
    }

    fn native_tools() -> PromptToolExposure<'static> {
        PromptToolExposure {
            native_tools_available: true,
            legacy_builtin_envelope_tools: &[],
        }
    }

    fn no_tools() -> PromptToolExposure<'static> {
        PromptToolExposure {
            native_tools_available: false,
            legacy_builtin_envelope_tools: &[],
        }
    }

    fn legacy_tools(names: &[&str]) -> PromptToolExposure<'static> {
        let names = names
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        PromptToolExposure {
            native_tools_available: false,
            legacy_builtin_envelope_tools: Box::leak(names.into_boxed_slice()),
        }
    }
}
