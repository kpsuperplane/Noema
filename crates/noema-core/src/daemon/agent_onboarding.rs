#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentPromptIdentity {
    pub agent_id: String,
    pub display_name: Option<String>,
}

pub(super) fn agent_identity_prompt(identity: &AgentPromptIdentity) -> String {
    let mut prompt = String::new();
    prompt.push_str("Agent identity:\n");
    prompt.push_str(&format!("- agent_id: {}\n", identity.agent_id));
    match identity
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        Some(name) => {
            prompt.push_str(&format!("- display_name: {name}\n"));
        }
        None => {
            prompt.push_str("- display_name: none\n\n");
            prompt.push_str("Onboarding prompt:\n");
            prompt.push_str("- You do not have a name yet.\n");
            prompt.push_str("- Your first priority is to ask the user to give you one.\n");
            prompt.push_str("- Do not invent, assume, or sign off with a name.\n");
            prompt
                .push_str("- If the user gives you a name, call update_own_name with that name.\n");
        }
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnamed_agent_prompt_includes_onboarding_prompt() {
        let prompt = agent_identity_prompt(&AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        });

        assert!(prompt.contains("Agent identity:"));
        assert!(prompt.contains("agent_id: agent:primary"));
        assert!(prompt.contains("display_name: none"));
        assert!(prompt.contains("Onboarding prompt:"));
        assert!(prompt.contains("You do not have a name yet."));
        assert!(prompt.contains("Your first priority is to ask the user to give you one."));
        assert!(prompt.contains("call update_own_name"));
    }

    #[test]
    fn named_agent_prompt_omits_missing_name_onboarding() {
        let prompt = agent_identity_prompt(&AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: Some("Mira".to_string()),
        });

        assert!(prompt.contains("Agent identity:"));
        assert!(prompt.contains("agent_id: agent:primary"));
        assert!(prompt.contains("display_name: Mira"));
        assert!(!prompt.contains("Onboarding prompt:"));
        assert!(!prompt.contains("You do not have a name yet."));
    }
}
